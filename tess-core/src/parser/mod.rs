use crate::{
    errors::ParserError,
    event_bus::EventBus,
    events::Event,
    parser::{
        constants::{PARSER_TYPE, ParserType},
        semantic_parser::SemanticParser,
    },
};

mod constants;
pub mod normalizer;
pub mod semantic_parser;

pub trait EventParser {
    fn parse(&self, bus: EventBus) -> Result<Vec<Event>, ParserError>;
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
    pub fn parse(&self, bus: EventBus) -> Result<Vec<Event>, ParserError> {
        self.inner.parse(bus)
    }

    pub fn init(&self) -> Result<(), ParserError> {
        self.inner.init()
    }

    // 4. Add a function to swap the inner logic at runtime
    pub fn swap_parser(&mut self, new_parser: Box<dyn EventParser>) {
        self.inner = new_parser;
    }
}
