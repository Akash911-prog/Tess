use std::sync::Arc;

use tokio::sync::broadcast;

use crate::events::TranscriptEvent;

#[derive(Debug, Clone)]
pub struct EventBus {
    pub sender: broadcast::Sender<Arc<TranscriptEvent>>,
}

impl EventBus {
    pub fn new(buffer_size: usize) -> Self {
        let (sender, _) = broadcast::channel::<Arc<TranscriptEvent>>(buffer_size);
        Self { sender }
    }

    pub fn publish(&self, event: TranscriptEvent) {
        tracing::debug!(event = ?event, "publishing transcript event");
        let _ = self.sender.send(Arc::new(event));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<TranscriptEvent>> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(200)
    }
}
