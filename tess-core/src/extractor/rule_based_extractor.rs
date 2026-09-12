use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use crate::{
    errors::ExtractorError,
    extractor::ArgExtractor,
    parser::{constants::COMMAND_FILLERS, normalizer::normalize_text},
    registry::{ArgKind, ArgSpec, ArgValue, SkillRegistry},
};

use regex::Regex;

fn digit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d+(?:\.\d+)?").unwrap())
}

fn word_numbers() -> &'static HashMap<&'static str, u64> {
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

pub struct RuleBasedExtractor {
    registry: Arc<SkillRegistry>,
}

impl RuleBasedExtractor {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }

    /// First number in `text`, as digits or a known number-word ("a" -> "1").
    fn find_number(&self, text: &str) -> Option<String> {
        if let Some(m) = digit_re().find(text) {
            return Some(m.as_str().to_string());
        }
        text.split_whitespace()
            .find_map(|tok| word_numbers().get(tok).map(|n| n.to_string()))
    }

    fn pre_process_text(&self, text: &str) -> String {
        let mut normalized = text.trim().to_lowercase();
        normalized = normalized
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() || c == '\'' || c == '.' {
                    c
                } else {
                    ' '
                }
            })
            .collect();

        normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

        loop {
            let mut changed = false;
            for filler in COMMAND_FILLERS {
                if let Some(rest) = normalized.strip_prefix(filler) {
                    if rest.is_empty() || rest.starts_with(' ') {
                        normalized = rest.trim_start().to_owned();
                        changed = true;
                        break;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        normalized
    }

    fn extract_duration(&self, text: &str) -> Option<String> {
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

    fn extract_integer(&self, text: &str) -> Option<String> {
        self.find_number(text)
    }

    fn extract_text(&self, text: &str) -> Option<String> {
        Some("".into())
    }

    fn extract_enum(&self, text: &str, allowed: &[&'static str]) -> Option<String> {
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
            let raw = match arg.kind {
                ArgKind::Text => self.extract_text(text),
                ArgKind::Integer => self.extract_integer(text),
                ArgKind::Duration => self.extract_duration(text),
                ArgKind::Enum(allowed) => self.extract_enum(text, &allowed),
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
