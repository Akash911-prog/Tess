use std::sync::Arc;

use crate::{
    errors::ParserError,
    events::{Event, TranscriptEvent},
    parser::{
        constants::{PARSER_TYPE, ParserType},
        semantic_parser::SemanticParser,
    },
};

mod constants;
pub mod normalizer;
pub mod semantic_parser;

pub trait EventParser: Send + Sync {
    fn parse(&self, event: Arc<TranscriptEvent>) -> Result<Vec<Event>, ParserError>;
    fn init(&self) -> Result<(), ParserError>;
}

pub struct Parser {
    inner: Box<dyn EventParser>,
}

impl Parser {
    pub fn new() -> Self {
        let inner: Box<dyn EventParser> = match PARSER_TYPE {
            ParserType::Semantic => Box::new(SemanticParser {}),
        };

        Parser { inner }
    }

    // 3. Delegate the trait methods through the cover struct
    pub fn parse(&self, event: Arc<TranscriptEvent>) -> Result<Vec<Event>, ParserError> {
        self.inner.parse(event)
    }

    pub fn init(&self) -> Result<(), ParserError> {
        self.inner.init()
    }

    // 4. Add a function to swap the inner logic at runtime
    pub fn swap_parser(&mut self, new_parser: Box<dyn EventParser>) {
        self.inner = new_parser;
    }
}
