use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    context_manager::{ContextManager, Mode},
    events::Event,
    registry::{skill::RiskLevel, SkillRegistry},
};

/// The outcome of a security policy evaluation.
///
/// The main event loop branches on this result to decide whether to execute,
/// prompt, authorise, or drop the command entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyCheckResult {
    /// The command is cleared to run.
    ///
    /// `silent` controls whether the system emits spoken/logged feedback.
    /// When `true` the caller should suppress all user-facing output from
    /// the execution (but still execute the action).
    Allow { silent: bool },

    /// The command requires a user-facing confirmation prompt before execution.
    ///
    /// The caller is responsible for surfacing `prompt` and re-attempting
    /// execution only after explicit approval.
    Confirmation { prompt: String },

    /// The command requires a privileged credential or UAC-style elevation.
    ///
    /// Reserved for future use; the caller should request the credential
    /// described in `prompt` before proceeding.
    Authorize { prompt: String },

    /// The command is unconditionally blocked.
    ///
    /// No execution, no feedback, no prompt.  Used for explicit user blocklists
    /// or future "do-not-disturb" modes that fully prevent a category of action.
    Deny,
}

/// Evaluates whether a parsed command is permitted to run given the current
/// system context (risk level × operating mode).
///
/// # Decision matrix
///
/// | Mode    | Low               | Medium            | High                      |
/// |---------|-------------------|-------------------|---------------------------|
/// | Default | Allow(silent: f)  | Allow(silent: f)  | Confirmation              |
/// | Media   | Allow(silent: t)  | Allow(silent: t)  | Confirmation              |
/// | Silent  | Allow(silent: t)  | Confirmation      | Allow(silent: f) ¹        |
/// | Debug   | Allow(silent: f)  | Allow(silent: f)  | Allow(silent: f)          |
///
/// ¹ **High-risk commands always break out of `Silent` mode.**
/// The user chose silence for ambient noise, not to mask potentially
/// destructive actions.  High-risk commands run with normal (non-silent)
/// feedback so the user is always aware they happened.
pub struct PolicyGate {
    registry: Arc<SkillRegistry>,
}

impl PolicyGate {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }

    /// Evaluates the security policy for a parsed `command` in the current context.
    ///
    /// # Arguments
    /// * `command`  — The fully-parsed, arg-extracted command about to be dispatched.
    /// * `context`  — The live `ContextManager` wrapped in an `RwLock`.
    pub async fn security_check(
        &self,
        command: &Event,
        context: Arc<RwLock<ContextManager>>,
    ) -> PolicyCheckResult {
        // Resolve risk level from the registry descriptor; default to High if
        // the intent is somehow unregistered (defence in depth).
        let risk_level = self
            .registry
            .descriptor_for(&command.intent)
            .map(|d| d.risk_level)
            .unwrap_or(RiskLevel::High);

        // Take a consistent snapshot so the rest of this function sees the
        // same mode/active_app even if another task mutates context mid-flight.
        let mode = context.read().await.mode;

        match (mode, risk_level) {
            // ── Default ──────────────────────────────────────────────────────
            (Mode::Default, RiskLevel::Low | RiskLevel::Medium) => {
                PolicyCheckResult::Allow { silent: false }
            }
            (Mode::Default, RiskLevel::High) => PolicyCheckResult::Confirmation {
                prompt: format!(
                    "\"{}\" is a high-risk action. Are you sure?",
                    command.intent
                ),
            },

            // ── Media ─────────────────────────────────────────────────────────
            // All low/medium commands run silently so they don't interrupt playback.
            (Mode::Media, RiskLevel::Low | RiskLevel::Medium) => {
                PolicyCheckResult::Allow { silent: true }
            }
            (Mode::Media, RiskLevel::High) => PolicyCheckResult::Confirmation {
                prompt: format!(
                    "\"{}\" is a high-risk action. Are you sure?",
                    command.intent
                ),
            },

            // ── Silent ────────────────────────────────────────────────────────
            // Low-risk commands run silently — no noise.
            (Mode::Silent, RiskLevel::Low) => PolicyCheckResult::Allow { silent: true },
            // Medium-risk gets a confirmation even in silent mode.
            (Mode::Silent, RiskLevel::Medium) => PolicyCheckResult::Confirmation {
                prompt: format!(
                    "\"{}\" needs your confirmation (medium-risk in silent mode).",
                    command.intent
                ),
            },
            // HIGH-RISK IGNORES SILENT MODE — always surfaces with confirmation.
            // The user chose silent to suppress ambient noise, not to mask
            // potentially destructive actions.
            (Mode::Silent, RiskLevel::High) => PolicyCheckResult::Confirmation {
                prompt: format!(
                    "\"{}\" is high-risk. Silent mode does not suppress this — confirm to proceed.",
                    command.intent
                ),
            },

            // ── Debug ─────────────────────────────────────────────────────────
            // Everything runs with full verbose feedback; no blocking at all.
            // This is intentional: debug mode is for developers who need to
            // observe the full execution path.
            (Mode::Debug, _) => PolicyCheckResult::Allow { silent: false },
        }
    }
}
