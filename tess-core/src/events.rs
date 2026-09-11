use std::fmt::Display;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SttTranscript,
}
#[derive(Debug, Deserialize)]
pub struct TranscriptEvent {
    pub schema_version: u8,
    pub event_type: EventType,
    pub trace_id: String,
    pub text: String,
}

impl Display for TranscriptEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}
