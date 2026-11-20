//! The argument checks every tool shares.
use lxagents_gitlab::models::args::*;
use serde_json::{Value, json};

use crate::helpers::assert_match;

fn refused<T: std::fmt::Debug>(
    result: Result<T, lxagents_gitlab::utils::errors::Refused>,
    pattern: &str,
) {
    match result {
        Ok(value) => panic!("expected a refusal matching /{pattern}/, got {value:?}"),
        Err(refused) => assert_match(&refused.0, pattern),
    }
}

const N: Value = Value::Null;

#[test]
fn optional_text_trims_drops_blanks_and_refuses_a_non_string() {
    assert_eq!(optional_text(&N, "x").unwrap(), None);
    assert_eq!(optional_text(&json!("   "), "x").unwrap(), None);
    assert_eq!(
        optional_text(&json!("  hi "), "x").unwrap().as_deref(),
        Some("hi")
    );
    refused(
        optional_text(&json!(5), "x"),
        r"`x` must be a string, got number\.",
    );
}

#[test]
fn required_text_refuses_a_missing_or_blank_value() {
    assert_eq!(required_text(&json!(" a "), "x").unwrap(), "a");
    for value in [N, json!(""), json!("  ")] {
        refused(
            required_text(&value, "x"),
            r"`x` is required and must be a non-empty string\.",
        );
    }
}

#[test]
fn raw_text_keeps_the_text_exactly_and_checks_only_that_it_is_a_string() {
    assert_eq!(
        raw_text(&json!("  keep\n"), "x").unwrap().as_deref(),
        Some("  keep\n")
    );
    assert_eq!(raw_text(&N, "x").unwrap(), None);
    refused(required_raw_text(&N, "x", false), r"`x` is required\.");
    refused(
        required_raw_text(&json!("  "), "x", false),
        "must not be blank",
    );
    assert_eq!(required_raw_text(&json!(""), "x", true).unwrap(), "");
    refused(
        raw_text(&json!(3), "x"),
        r"`x` must be a string, got number\.",
    );
}

#[test]
fn optional_flag_falls_back_when_absent_and_refuses_a_non_boolean() {
    assert!(optional_flag(&N, "x", true).unwrap());
    assert!(!optional_flag(&N, "x", false).unwrap());
    assert!(!optional_flag(&json!(false), "x", true).unwrap());
    refused(
        optional_flag(&json!("yes"), "x", true),
        r"`x` must be true or false, got string\.",
    );
}

#[test]
fn optional_int_and_required_int_take_whole_numbers_inside_the_range() {
    let range = IntRange::min(1);
    assert_eq!(optional_int(&N, "n", range).unwrap(), None);
    assert_eq!(
        optional_int(&json!(5), "n", IntRange::between(1, 10)).unwrap(),
        Some(5)
    );
    refused(
        optional_int(&json!(1.5), "n", range),
        r"`n` must be a whole number, got 1\.5\.",
    );
    refused(
        optional_int(&json!("3"), "n", range),
        r#"must be a whole number, got "3""#,
    );
    refused(
        optional_int(&json!(0), "n", range),
        r"`n` must be between 1 and",
    );
    refused(
        optional_int(&json!(11), "n", IntRange::between(1, 10)),
        r"`n` must be between 1 and 10, got 11\.",
    );
    assert_eq!(required_int(&json!(7), "n", range).unwrap(), 7);
    refused(
        required_int(&N, "n", range),
        r"`n` is required and must be a whole number\.",
    );
}

#[test]
fn one_of_returns_the_fallback_when_absent_and_refuses_a_word_outside_the_set() {
    let set = &["a", "b"];
    assert_eq!(
        one_of(&N, "m", set, Some("a")).unwrap().as_deref(),
        Some("a")
    );
    assert_eq!(
        one_of(&json!(" b "), "m", set, None).unwrap().as_deref(),
        Some("b")
    );
    assert_eq!(one_of(&N, "m", set, None).unwrap(), None);
    refused(
        one_of(&json!("c"), "m", set, None),
        r"`m` must be one of a, b, got 'c'\.",
    );
}

#[test]
fn text_list_trims_drops_blanks_and_duplicates_and_enforces_its_bounds() {
    let loose = ListOptions {
        required: false,
        max: 100,
    };
    let required = ListOptions {
        required: true,
        max: 100,
    };
    assert_eq!(
        text_list(&json!([" a ", "b", "a", "  "]), "l", loose).unwrap(),
        Some(vec!["a".into(), "b".into()])
    );
    assert_eq!(text_list(&N, "l", loose).unwrap(), None);
    refused(text_list(&N, "l", required), "`l` is required");
    refused(text_list(&json!(["  "]), "l", required), "`l` is required");
    refused(
        text_list(&json!("a"), "l", loose),
        r"`l` must be a list of strings\.",
    );
    refused(
        text_list(&json!([1]), "l", loose),
        r"`l` must be a list of strings\.",
    );
    refused(
        text_list(
            &json!(["a", "b", "c"]),
            "l",
            ListOptions {
                required: false,
                max: 2,
            },
        ),
        "at most 2 values, got 3",
    );
}

#[test]
fn maybe_flag_keeps_not_given_apart_from_false() {
    assert_eq!(maybe_flag(&N, "x").unwrap(), None);
    assert_eq!(maybe_flag(&json!(false), "x").unwrap(), Some(false));
    assert_eq!(maybe_flag(&json!(true), "x").unwrap(), Some(true));
    refused(
        maybe_flag(&json!("no"), "x"),
        r"`x` must be true or false, got string\.",
    );
}

#[test]
fn optional_date_takes_an_iso_8601_date_or_timestamp_and_refuses_anything_else() {
    assert_eq!(optional_date(&N, "since").unwrap(), None);
    assert_eq!(optional_date(&json!("  "), "since").unwrap(), None);
    assert_eq!(
        optional_date(&json!("2026-10-01"), "since")
            .unwrap()
            .as_deref(),
        Some("2026-10-01")
    );
    assert_eq!(
        optional_date(&json!(" 2026-10-01T12:00:00Z "), "since")
            .unwrap()
            .as_deref(),
        Some("2026-10-01T12:00:00Z")
    );
    for bad in ["yesterday", "10/01/2026", "2026-13-45", "2026-1-1"] {
        refused(
            optional_date(&json!(bad), "since"),
            "`since` must be an ISO 8601 date",
        );
    }
    refused(
        optional_date(&json!(5), "since"),
        "`since` must be a string",
    );
}
