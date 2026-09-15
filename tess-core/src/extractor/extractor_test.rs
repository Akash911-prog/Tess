use super::*;
use crate::registry::{ArgKind, ArgSpec, ExecutionResult};
use std::time::Duration;

// A small in-test skill exposing a handful of intents that exercise every
// `ArgKind`, plus a "mixed" intent used to probe how `extract()` behaves
// when multiple args are declared together.
crate::skill! {
    struct TestSkill;
    name = "test";

    intent "test.no_args" {
        desc: "takes no arguments",
        args: [],
        exemplars: ["do nothing"],
    }

    intent "test.required_int" {
        desc: "requires an integer level",
        args: [ArgSpec::required("level", ArgKind::Integer)],
        exemplars: ["set level"],
    }

    intent "test.optional_duration" {
        desc: "accepts an optional duration",
        args: [ArgSpec::optional("duration", ArgKind::Duration)],
        exemplars: ["skip"],
    }

    intent "test.enum_toggle" {
        desc: "accepts an optional on/off toggle",
        args: [ArgSpec::optional("mode", ArgKind::Enum(&["on", "off"]))],
        exemplars: ["toggle"],
    }

    intent "test.required_text" {
        desc: "requires free-form text",
        args: [ArgSpec::required("query", ArgKind::Text)],
        exemplars: ["search"],
    }

    intent "test.mixed" {
        desc: "required integer + optional enum; used to probe extraction/parse ordering",
        args: [
            ArgSpec::required("level", ArgKind::Integer),
            ArgSpec::optional("mode", ArgKind::Enum(&["on", "off"])),
        ],
        exemplars: ["mixed"],
    }

    execute(_command) {
        Ok(ExecutionResult::success())
    }
}

fn extractor() -> RuleBasedExtractor {
    let mut registry = SkillRegistry::new();
    registry.register(TestSkill).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();

    runtime.block_on(RuleBasedExtractor::new(Arc::new(registry)))
}

// ---------------------------------------------------------------- //
// find_number
// ---------------------------------------------------------------- //

#[test]
fn find_number_prefers_digits_over_words() {
    let ex = extractor();
    assert_eq!(ex.find_number("set it to 5 please"), Some("5".to_string()));
}

#[test]
fn find_number_parses_decimals() {
    let ex = extractor();
    assert_eq!(ex.find_number("wait 2.5 seconds"), Some("2.5".to_string()));
}

#[test]
fn find_number_falls_back_to_number_words() {
    let ex = extractor();
    assert_eq!(ex.find_number("wait a minute"), Some("1".to_string()));
    assert_eq!(
        ex.find_number("give me a couple of minutes"),
        Some("2".to_string())
    );
    assert_eq!(
        ex.find_number("go back twenty seconds"),
        Some("20".to_string())
    );
}

#[test]
fn find_number_is_case_sensitive_on_words() {
    // Word-number lookup does not lowercase tokens first, so capitalized
    // words won't match the (lowercase-only) `word_numbers` table.
    let ex = extractor();
    assert_eq!(ex.find_number("wait A minute"), None);
}

#[test]
fn find_number_returns_none_when_nothing_found() {
    let ex = extractor();
    let result = ex.find_number("please help me");
    assert_eq!(result, None);
}

// ---------------------------------------------------------------- //
// extract_duration
// ---------------------------------------------------------------- //

#[test]
fn extract_duration_recognizes_hours() {
    let ex = extractor();
    assert_eq!(
        ex.extract_duration("remind me in 2 hours"),
        Some("2 hours".to_string())
    );
    assert_eq!(ex.extract_duration("in an hr"), Some("1 hours".to_string()));
}

#[test]
fn extract_duration_recognizes_minutes() {
    let ex = extractor();
    assert_eq!(
        ex.extract_duration("skip 10 min"),
        Some("10 minutes".to_string())
    );
}

#[test]
fn extract_duration_defaults_to_seconds() {
    let ex = extractor();
    assert_eq!(
        ex.extract_duration("rewind 30"),
        Some("30 seconds".to_string())
    );
}

#[test]
fn extract_duration_none_without_a_number() {
    let ex = extractor();
    assert_eq!(ex.extract_duration("skip ahead"), None);
}

// ---------------------------------------------------------------- //
// extract_integer
// ---------------------------------------------------------------- //

#[test]
fn extract_integer_from_digits() {
    let ex = extractor();
    assert_eq!(
        ex.extract_integer("set volume to 42"),
        Some("42".to_string())
    );
}

#[test]
fn extract_integer_from_number_words() {
    let ex = extractor();
    assert_eq!(
        ex.extract_integer("set volume to three"),
        Some("3".to_string())
    );
}

#[test]
fn extract_integer_none_when_no_number_present() {
    let ex = extractor();
    assert_eq!(ex.extract_integer("turn the volume up"), None);
}

// ---------------------------------------------------------------- //
// extract_enum
// ---------------------------------------------------------------- //

#[test]
fn extract_enum_matches_a_contained_substring() {
    let ex = extractor();
    assert_eq!(
        ex.extract_enum("turn wifi on please", &["on", "off"]),
        Some("on".to_string())
    );
}

#[test]
fn extract_enum_none_when_nothing_matches() {
    let ex = extractor();
    assert_eq!(ex.extract_enum("toggle it", &["on", "off"]), None);
}

#[test]
fn extract_enum_picks_whichever_allowed_value_is_checked_first() {
    // `extract_enum` short-circuits on the *first* entry of `allowed` that is
    // a substring of `text`, regardless of where each candidate actually
    // appears in the text. Both "on" and "off" occur here, so the winner is
    // whichever comes first in the `allowed` slice, not in the sentence.
    let ex = extractor();
    assert_eq!(
        ex.extract_enum("switch off then on", &["on", "off"]),
        Some("on".to_string())
    );
    assert_eq!(
        ex.extract_enum("switch off then on", &["off", "on"]),
        Some("off".to_string())
    );
}

// ---------------------------------------------------------------- //
// extract_text
// ---------------------------------------------------------------- //

#[test]
fn extract_text_currently_always_returns_empty_string() {
    // `extract_text` is a stub: it ignores its `text` argument entirely and
    // always returns `Some("")`. This test pins down the current behavior so
    // a future real implementation changes it deliberately, not silently.
    let ex = extractor();
    assert_eq!(
        ex.extract_text("play bohemian rhapsody"),
        Some("".to_string())
    );
    assert_eq!(ex.extract_text(""), Some("".to_string()));
}

// ---------------------------------------------------------------- //
// extract() end-to-end (via the public ArgExtractor trait)
// ---------------------------------------------------------------- //

#[test]
fn extract_errors_on_unknown_intent() {
    let ex = extractor();
    let err = ex.extract("nope.nope", "anything").unwrap_err();
    assert!(matches!(err, ExtractorError::NoIntent(id) if id == "nope.nope"));
}

#[test]
fn extract_with_no_declared_args_returns_empty_map() {
    let ex = extractor();
    let result = ex.extract("test.no_args", "do nothing at all").unwrap();
    assert!(result.is_empty());
}

#[test]
fn extract_required_integer_succeeds() {
    let ex = extractor();
    let result = ex.extract("test.required_int", "set level to 7").unwrap();
    assert_eq!(result.get("level"), Some(&ArgValue::Integer(7)));
}

#[test]
fn extract_required_integer_errors_when_no_number_present() {
    let ex = extractor();
    // No digits or number-words anywhere, so `extract_integer` yields None,
    // "level" is never added to `raw_args`, and `ArgSpec::parse_all` reports
    // it missing.
    let err = ex
        .extract("test.required_int", "set the level please")
        .unwrap_err();
    assert!(matches!(err, ExtractorError::Extract(_)));
}

#[test]
fn extract_optional_duration_with_word_number() {
    let ex = extractor();
    let result = ex
        .extract("test.optional_duration", "skip ahead a minute")
        .unwrap();
    assert_eq!(
        result.get("duration"),
        Some(&ArgValue::Duration(Duration::from_secs(60)))
    );
}

#[test]
fn extract_optional_duration_missing_is_fine() {
    let ex = extractor();
    let result = ex.extract("test.optional_duration", "just skip").unwrap();
    assert!(result.is_empty());
}

#[test]
fn extract_enum_toggle_picks_allowed_value() {
    let ex = extractor();
    let result = ex.extract("test.enum_toggle", "turn it off").unwrap();
    assert_eq!(result.get("mode"), Some(&ArgValue::Enum("off")));
}

#[test]
fn extract_required_text_yields_empty_value_due_to_stub() {
    // Because `extract_text` always returns `Some("")`, a *required* Text
    // arg never fails extraction — but it also never captures anything the
    // user actually said.
    let ex = extractor();
    let result = ex
        .extract("test.required_text", "search for lo-fi beats")
        .unwrap();
    assert_eq!(result.get("query"), Some(&ArgValue::Text("".to_string())));
}

#[test]
fn extract_mixed_args_succeed_when_every_arg_extracts() {
    let ex = extractor();
    let result = ex.extract("test.mixed", "set level 5 mode on").unwrap();
    assert_eq!(result.get("level"), Some(&ArgValue::Integer(5)));
    assert_eq!(result.get("mode"), Some(&ArgValue::Enum("on")));
}

#[test]
fn extract_mixed_args_can_misalign_when_an_earlier_arg_fails_to_extract() {
    // Known limitation: `extract()` builds `raw_args` by pushing only the
    // values that were *successfully* extracted, then zips that flat,
    // compacted list against `specs` positionally inside
    // `ArgSpec::parse_all`. If an earlier spec's extraction fails but a
    // later one succeeds, the later value slides into the earlier slot
    // instead of being dropped or matched up by name.
    //
    // Here "level" (Integer, required) finds no number in the text and is
    // skipped, but "mode" (Enum, optional) matches "on". The lone raw value
    // "on" then gets zipped against the *first* spec, "level", which fails
    // to parse "on" as an integer — so this errors, but for the wrong
    // reason (InvalidValue on "level", not a clean "mode": on" result).
    let ex = extractor();
    let err = ex.extract("test.mixed", "please turn it on").unwrap_err();
    assert!(matches!(err, ExtractorError::Extract(_)));
}
