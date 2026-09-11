use crate::{errors::ParserError, event_bus::EventBus, events::Event, parser::EventParser};

pub struct SemanticParser {}

impl EventParser for SemanticParser {
    fn parse(&self, bus: EventBus) -> Result<Vec<Event>, ParserError> {
        todo!()
    }

    fn init(&self) -> Result<(), ParserError> {
        todo!()
    }
}
