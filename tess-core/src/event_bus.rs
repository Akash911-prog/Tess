use crate::events::TranscriptEvent;

#[derive(Debug, Clone, Copy)]
pub struct EventBus {}

impl EventBus {
    pub fn new() -> Self {
        Self {}
    }

    pub fn publish(&self, event: TranscriptEvent) {
        tracing::debug!(event = ?event, "publishing transcript event");
    }
}
