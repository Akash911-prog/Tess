pub mod bio_tagger;

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use crate::{
    errors::ExtractorError,
    extractor::{ArgExtractor, rule_based_extractor::bio_tagger::BioTagger},
    registry::{ArgKind, ArgSpec, ArgValue, SkillRegistry},
};

use regex::Regex;

pub fn digit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d+(?:\.\d+)?").unwrap())
}

pub fn word_numbers() -> &'static HashMap<&'static str, u64> {
    static MAP: OnceLock<HashMap<&'static str, u64>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("a", 1),
            ("an", 1),
            ("one", 1),
            ("couple", 2),
            ("two", 2),
            ("three", 3),
            ("four", 4),
            ("five", 5),
            ("ten", 10),
            ("twenty", 20),
            ("thirty", 30),
        ])
    })
}

/// `"a"`/`"an"` are indefinite articles that only mean "one" when nothing more
/// specific is said (e.g. "wait a minute"). They should never outrank an actual
/// quantity word like "couple" or "twenty" just because they happen to appear
/// earlier in the sentence (e.g. "a couple of minutes" means 2, not 1).
fn is_indefinite_article(tok: &str) -> bool {
    tok == "a" || tok == "an"
}

pub struct RuleBasedExtractor {
    registry: Arc<SkillRegistry>,
    tagger: BioTagger,
}

impl RuleBasedExtractor {
    pub async fn new(registry: Arc<SkillRegistry>) -> Self {
        let tagger = BioTagger::new().await;
        Self { registry, tagger }
    }

    /// First number in `text`, as digits or a known number-word ("a" -> "1").
    ///
    /// Explicit number-words ("couple", "two", "twenty", ...) win over the
    /// indefinite articles "a"/"an" regardless of which comes first in the
    /// sentence, since "a"/"an" only stand in for "one" when nothing more
    /// specific is present (see [`is_indefinite_article`]).
    pub fn find_number(&self, text: &str) -> Option<String> {
        if let Some(m) = digit_re().find(text) {
            return Some(m.as_str().to_string());
        }

        let tokens: Vec<&str> = text.split_whitespace().collect();

        let specific = tokens.iter().find_map(|&tok| {
            if is_indefinite_article(tok) {
                return None;
            }
            word_numbers().get(tok).map(|n| n.to_string())
        });
        if specific.is_some() {
            return specific;
        }

        tokens.iter().find_map(|&tok| {
            if !is_indefinite_article(tok) {
                return None;
            }
            word_numbers().get(tok).map(|n| n.to_string())
        })
    }
    pub fn extract_duration(&self, text: &str) -> Option<String> {
        let amount = self.find_number(text)?;

        let unit = if text.contains("hour") || text.contains("hr") {
            "hours"
        } else if text.contains("min") {
            "minutes"
        } else {
            "seconds"
        };

        // Shape parse_duration in registry/args.rs expects: "<amount> <unit>".
        Some(format!("{amount} {unit}"))
    }

    pub fn extract_integer(&self, text: &str) -> Option<String> {
        self.find_number(text)
    }

    pub fn extract_text(&self, text: &str) -> Option<String> {
        let tag = self.tagger.tag(text);
        Some(tag.join(" ")) //TODO: remove whitespace
    }

    pub fn extract_enum(&self, text: &str, allowed: &[&'static str]) -> Option<String> {
        if let Some(matched_word) = allowed.iter().find(|&&sub| text.contains(sub)) {
            return Some(matched_word.to_string());
        } else {
            return None;
        }
    }
}

impl ArgExtractor for RuleBasedExtractor {
    fn extract(
        &self,
        intent: &str,
        text: &str,
    ) -> Result<HashMap<&'static str, ArgValue>, ExtractorError> {
        let Some(descriptor) = self.registry.descriptor_for(intent) else {
            return Err(ExtractorError::NoIntent(intent.to_string()));
        };

        let mut raw_args: Vec<String> = vec![];

        let args = descriptor.args;

        if args.is_empty() {
            return Ok(HashMap::new());
        }

        for arg in args {
            let text = text.trim().to_lowercase();
            let raw = match arg.kind {
                ArgKind::Text => self.extract_text(&text),
                ArgKind::Integer => self.extract_integer(&text),
                ArgKind::Duration => self.extract_duration(&text),
                ArgKind::Enum(allowed) => self.extract_enum(&text, &allowed),
            };

            if let Some(raw) = raw {
                raw_args.push(raw);
            }
        }

        let result = ArgSpec::parse_all(args, &raw_args)
            .map_err(|_| ExtractorError::Extract(anyhow::anyhow!("failed to parse args")))?;

        Ok(result)
    }
}
