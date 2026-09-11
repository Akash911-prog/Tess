use std::{ops::Deref, sync::Arc};

use crate::{
    errors::ParserError,
    events::{Event, TranscriptEvent},
    parser::semantic_parser::SemanticParser,
    registry::IntentDescriptor,
};

mod constants;
pub mod engine;
pub mod normalizer;
pub mod semantic_parser;

pub use engine::{EmbeddingEngine, FastEmbedEngine, ModelSource};
pub use semantic_parser::{DEFAULT_MIN_MARGIN, DEFAULT_SIMILARITY_THRESHOLD, ExemplarEmbedding};

pub trait EventParser: Send + Sync {
    fn parse(&self, event: Arc<TranscriptEvent>) -> Result<Vec<Event>, ParserError>;
    fn init(&self) -> Result<(), ParserError>;
    fn load_catalog(&self, _catalog: &[IntentDescriptor]) -> Result<(), ParserError> {
        Ok(())
    }
}

pub struct Parser {
    inner: Box<dyn EventParser>,
}

impl Parser {
    pub fn new(inner: Box<dyn EventParser>) -> Self {
        Self { inner }
    }

    pub fn semantic() -> Self {
        Self::new(Box::new(SemanticParser::default()))
    }

    pub fn load_catalog(&self, catalog: &[IntentDescriptor]) -> Result<(), ParserError> {
        self.inner.load_catalog(catalog)
    }

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
