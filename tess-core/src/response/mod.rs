/// The contract for delivering feedback to the user after a skill executes.
///
/// Implementors decide *how* the feedback surfaces: logging to stdout, TTS,
/// a system notification, an IPC message back to the UI, etc.
///
/// Under the Open-Closed Principle this module is the stable abstraction;
/// new delivery mechanisms are added as new implementors, never by modifying
/// existing ones.
pub trait ResponseSink: Send + Sync {
    /// Deliver a feedback message to the user.
    fn speak(&self, text: &str);
}

// ─── Logging sink (always compiled in) ───────────────────────────────────────

/// A `ResponseSink` that emits feedback through the `tracing` logging layer.
///
/// This is the default sink used until a TTS or UI backend is wired in.
/// It is zero-allocation and always available regardless of the platform.
pub struct LoggingResponseSink;

impl ResponseSink for LoggingResponseSink {
    fn speak(&self, text: &str) {
        tracing::info!(feedback = %text, "tess response");
    }
}
