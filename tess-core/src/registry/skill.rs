use std::future::Future;
use std::pin::Pin;

use crate::events::Event;
use crate::registry::args::ArgSpec;

/// Type alias for pinned, heap-allocated futures returned by object-safe async trait methods.
///
/// This eliminates the need for third-party procedural macros while ensuring zero-cost
/// extensibility and complete dynamic dispatch (`dyn Skill`) safety.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Metadata and training exemplar phrases defining a single intent.
///
/// Under the Open-Closed Principle, skills define their own intent descriptors.
/// The core engine and parser never hardcode intents; they discover and index them
/// through these descriptors at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentDescriptor {
    /// Unique identifier in the `domain.action` format (e.g., `"media.pause"`).
    pub id: &'static str,

    /// Human-readable explanation of what this intent accomplishes.
    pub description: &'static str,

    /// The typed argument schema this intent accepts. Use
    /// [`ArgSpec::parse_all`](crate::registry::ArgSpec::parse_all) inside `execute`
    /// to validate and coerce an `Event`'s raw args against this schema.
    pub args: &'static [ArgSpec],

    /// Canonical anchor phrases and user utterances used by the semantic parser
    /// to generate vector embeddings for cosine similarity matching.
    pub exemplars: &'static [&'static str],
}

impl IntentDescriptor {
    /// Creates a new immutable intent descriptor.
    pub const fn new(
        id: &'static str,
        description: &'static str,
        args: &'static [ArgSpec],
        exemplars: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            description,
            args,
            exemplars,
        }
    }
}

/// The result returned after a skill successfully executes a command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionResult {
    /// Optional natural language feedback or response text to be spoken
    /// back to the user or displayed in UI notifications.
    pub feedback: Option<String>,
}

impl ExecutionResult {
    /// Produces a silent success result with no spoken or visual feedback.
    pub fn success() -> Self {
        Self { feedback: None }
    }

    /// Produces a success result with user-facing speech or text feedback.
    pub fn with_feedback(feedback: impl Into<String>) -> Self {
        Self {
            feedback: Some(feedback.into()),
        }
    }
}

/// The core extension contract for all Tess capabilities.
///
/// # Extensibility
/// Any new capability (media control, application launching, system shortcuts, etc.)
/// must implement `Skill`. Once implemented, register it in `SkillRegistry`.
///
/// **Zero lines of core parsing or IPC infrastructure code need to change.**
pub trait Skill: Send + Sync {
    /// The unique domain or namespace of this skill (e.g., `"media"`, `"system"`, `"app"`).
    fn name(&self) -> &'static str;

    /// Returns all intent descriptors provided by this skill.
    ///
    /// The registry collects these at startup and provides them to the semantic parser
    /// to build the embedding similarity catalog.
    fn intents(&self) -> Vec<IntentDescriptor>;

    /// Executes the matched command asynchronously.
    ///
    /// The implementation should handle the domain logic and return either an `ExecutionResult`
    /// or propagate errors cleanly using `anyhow::Error`.
    fn execute<'a>(
        &'a self,
        command: &'a Event,
    ) -> BoxFuture<'a, Result<ExecutionResult, anyhow::Error>>;
}
