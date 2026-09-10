use std::fmt::Display;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TranscriptEvent {
    pub trace_id: String,
    pub text: String,
    pub confidence: f32,
}

impl Display for TranscriptEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}
