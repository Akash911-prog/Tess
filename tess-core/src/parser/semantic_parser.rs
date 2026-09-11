use std::sync::Arc;

use anyhow::Ok;

use crate::{
    errors::ParserError,
    events::{Event, TranscriptEvent},
    parser::EventParser,
};

pub struct SemanticParser {}

impl EventParser for SemanticParser {
    fn parse(&self, event: Arc<TranscriptEvent>) -> Result<Vec<Event>, ParserError> {
        todo!()
    }

    fn init(&self) -> Result<(), ParserError> {
        todo!()
    }
}
