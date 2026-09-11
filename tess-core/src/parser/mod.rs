use std::{ops::Deref, sync::Arc};

use crate::{
    errors::ParserError,
    events::{Event, TranscriptEvent},
    parser::semantic_parser::SemanticParser,
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
    pub fn new(inner: Box<dyn EventParser>) -> Self {
        Self { inner }
    }

    fn semantic() -> Self {
        Self::new(Box::new(SemanticParser {}))
    }

    // 4. Add a function to swap the inner logic at runtime
    pub fn swap_parser(&mut self, new_parser: Box<dyn EventParser>) {
        self.inner = new_parser;
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::semantic()
    }
}

impl Deref for Parser {
    type Target = dyn EventParser;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}
