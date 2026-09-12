mod rule_based_extractor;

#[cfg(test)]
mod extractor_test;

use std::{collections::HashMap, ops::Deref, sync::Arc};

use crate::{
    errors::ExtractorError,
    extractor::rule_based_extractor::RuleBasedExtractor,
    registry::{ArgValue, SkillRegistry},
};

pub trait ArgExtractor: Send + Sync {
    fn extract(
        &self,
        intent: &str,
        text: &str,
    ) -> Result<HashMap<&'static str, ArgValue>, ExtractorError>;
}

pub struct Extractor {
    pub extractor: Box<dyn ArgExtractor>,
}

impl Extractor {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        let inner = Box::new(RuleBasedExtractor::new(registry));
        Self { extractor: inner }
    }
}

impl Deref for Extractor {
    type Target = dyn ArgExtractor;

    fn deref(&self) -> &Self::Target {
        &*self.extractor
    }
}
