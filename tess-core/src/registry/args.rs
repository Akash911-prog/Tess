//! Typed argument schema for intents.
//!
//! Previously, [`IntentDescriptor`](crate::registry::IntentDescriptor) declared its
//! arguments as free-form documentation strings (e.g. `"optional: duration"`) that
//! nothing ever parsed or validated. [`ArgSpec`] replaces that with a real,
//! compiler-checked schema: a name, an [`ArgKind`], and whether it's required.
//! [`ArgSpec::parse_all`] turns a skill's raw positional arguments into typed
//! [`ArgValue`]s against that schema, so a skill's `execute` can pull out a
//! `Duration` or an `i64` directly instead of hand-parsing strings.
//!
//! Adding a new argument shape is the only place this module needs to grow
//! (a new [`ArgKind`] variant); every skill that declares its arguments through
//! [`ArgSpec`] keeps working unchanged.

use std::collections::HashMap;
use std::time::Duration;

use thiserror::Error;

/// The shape of a single argument value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    /// Free-form text, taken as-is.
    Text,
    /// A whole number (e.g. a volume level).
    Integer,
    /// A span of time (e.g. "20 seconds" -> `Duration::from_secs(20)`).
    Duration,
    /// One of a fixed, closed set of values (e.g. an on/off toggle).
    Enum(&'static [&'static str]),
}

/// A typed, parsed argument value, produced by [`ArgSpec::parse_all`].
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    Text(String),
    Integer(i64),
    Duration(Duration),
    Enum(&'static str),
}

/// Declares one named argument an intent accepts, and whether it must be present.
///
/// `ArgSpec` construction is `const`, so descriptors built from `ArgSpec`s remain
/// eligible for the same `'static` rvalue promotion the rest of the registry relies
/// on (see [`IntentDescriptor::new`](crate::registry::IntentDescriptor::new)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    pub required: bool,
}

/// Errors produced while validating raw arguments against an [`ArgSpec`] schema.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ArgError {
    #[error("missing required argument '{name}'")]
    Missing { name: &'static str },

    #[error("argument '{name}' has invalid value '{raw}' for its declared type")]
    InvalidValue { name: &'static str, raw: String },
}

impl ArgSpec {
    /// Declares a required argument.
    pub const fn required(name: &'static str, kind: ArgKind) -> Self {
        Self {
            name,
            kind,
            required: true,
        }
    }

    /// Declares an optional argument.
    pub const fn optional(name: &'static str, kind: ArgKind) -> Self {
        Self {
            name,
            kind,
            required: false,
        }
    }

    /// Validates and coerces a positional list of raw argument tokens against a full
    /// intent's argument schema, matched in declaration order.
    ///
    /// Extra raw tokens beyond the declared specs are ignored. A missing token for a
    /// required spec, or a token that doesn't fit its declared [`ArgKind`], is an error.
    pub fn parse_all(
        specs: &[ArgSpec],
        raw: &[String],
    ) -> Result<HashMap<&'static str, ArgValue>, ArgError> {
        let mut parsed = HashMap::with_capacity(specs.len());

        for (spec, token) in specs.iter().zip(raw.iter()) {
            parsed.insert(spec.name, spec.parse(token)?);
        }

        for spec in specs.iter().skip(raw.len()) {
            if spec.required {
                return Err(ArgError::Missing { name: spec.name });
            }
        }

        Ok(parsed)
    }

    /// Parses a single raw token against this spec's [`ArgKind`].
    fn parse(&self, raw: &str) -> Result<ArgValue, ArgError> {
        let invalid = || ArgError::InvalidValue {
            name: self.name,
            raw: raw.to_string(),
        };

        match self.kind {
            ArgKind::Text => Ok(ArgValue::Text(raw.to_string())),
            ArgKind::Integer => raw.parse::<i64>().map(ArgValue::Integer).map_err(|_| invalid()),
            ArgKind::Duration => parse_duration(raw).map(ArgValue::Duration).ok_or_else(invalid),
            ArgKind::Enum(allowed) => allowed
                .iter()
                .copied()
                .find(|candidate| candidate.eq_ignore_ascii_case(raw))
                .map(ArgValue::Enum)
                .ok_or_else(invalid),
        }
    }
}

/// Parses a deliberately small `<amount> <unit>` grammar (e.g. `"20 seconds"`,
/// `"1 minute"`, `"30s"`) into a [`Duration`].
///
/// This intentionally does not attempt general natural-language number parsing
/// (e.g. "a minute") — normalizing spoken text into that shape is the parser's job,
/// not this schema's.
fn parse_duration(raw: &str) -> Option<Duration> {
    let raw = raw.trim().to_ascii_lowercase();
    let mut tokens = raw.split_whitespace();
    let first = tokens.next()?;

    // Accept both spaced ("20 seconds") and compact ("20s") forms by splitting the
    // first token into its leading digits and trailing unit when there's no second
    // token to serve as the unit.
    let (amount_str, unit) = match tokens.next() {
        Some(unit) => (first, unit),
        None => {
            let split_at = first
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(first.len());
            let (amount, unit) = first.split_at(split_at);
            (amount, if unit.is_empty() { "s" } else { unit })
        }
    };

    let amount: u64 = amount_str.parse().ok()?;

    let seconds = match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => amount,
        "m" | "min" | "mins" | "minute" | "minutes" => amount * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => amount * 3600,
        _ => return None,
    };

    Some(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_fills_required_and_optional_args() {
        const SPECS: &[ArgSpec] = &[
            ArgSpec::required("level", ArgKind::Integer),
            ArgSpec::optional("mode", ArgKind::Enum(&["on", "off"])),
        ];

        let raw = vec!["7".to_string(), "on".to_string()];
        let parsed = ArgSpec::parse_all(SPECS, &raw).unwrap();

        assert_eq!(parsed.get("level"), Some(&ArgValue::Integer(7)));
        assert_eq!(parsed.get("mode"), Some(&ArgValue::Enum("on")));
    }

    #[test]
    fn test_parse_all_missing_required_arg_errors() {
        const SPECS: &[ArgSpec] = &[ArgSpec::required("level", ArgKind::Integer)];

        let err = ArgSpec::parse_all(SPECS, &[]).unwrap_err();
        assert_eq!(err, ArgError::Missing { name: "level" });
    }

    #[test]
    fn test_parse_all_missing_optional_arg_is_fine() {
        const SPECS: &[ArgSpec] = &[ArgSpec::optional("duration", ArgKind::Duration)];

        let parsed = ArgSpec::parse_all(SPECS, &[]).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_parse_duration_units() {
        assert_eq!(parse_duration("20 seconds"), Some(Duration::from_secs(20)));
        assert_eq!(parse_duration("1 minute"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("not a duration"), None);
    }

    #[test]
    fn test_invalid_integer_errors() {
        const SPECS: &[ArgSpec] = &[ArgSpec::required("level", ArgKind::Integer)];
        let raw = vec!["loud".to_string()];

        let err = ArgSpec::parse_all(SPECS, &raw).unwrap_err();
        assert_eq!(
            err,
            ArgError::InvalidValue {
                name: "level",
                raw: "loud".to_string()
            }
        );
    }
}
