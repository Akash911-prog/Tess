use std::sync::Arc;

use tokio::sync::broadcast;

use crate::events::TranscriptEvent;

#[derive(Debug, Clone)]
pub struct EventBus {
    pub sender: broadcast::Sender<Arc<TranscriptEvent>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel::<Arc<TranscriptEvent>>(10);
        Self { sender }
    }

    pub fn publish(&self, event: TranscriptEvent) {
        tracing::debug!(event = ?event, "publishing transcript event");
        let _ = self.sender.send(Arc::new(event));
    }
}
