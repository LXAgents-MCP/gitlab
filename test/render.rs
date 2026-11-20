//! The text a model reads.
use lxagents_gitlab::config::config::MAX_OUTPUT;
use lxagents_gitlab::utils::render::{describe, fence, render_payload};
use lxagents_gitlab::utils::text::{
    cut_text, decode_text, first_line, head, looks_binary, short, tail,
};
use serde_json::{Map, Value, json};

fn render(summary: &str, fields: Value) -> String {
    let map: Map<String, Value> = fields.as_object().cloned().unwrap();
    render_payload(summary, &map)
}

#[test]
fn a_flat_payload_is_a_summary_and_one_bullet_per_scalar_in_order() {
    assert_eq!(
        render(
            "Created it.",
            json!({ "name": "demo", "count": 3, "draft": false })
        ),
        "**Created it.**\n\n- name: demo\n- count: 3\n- draft: false"
    );
}

#[test]
fn absent_fields_empty_lists_and_nested_objects_are_left_out_of_the_text() {
    assert_eq!(
        render(
            "S",
            json!({ "b": null, "c": [], "d": { "nested": 1 }, "e": "kept" })
        ),
        "**S**\n\n- e: kept"
    );
}

#[test]
fn a_list_of_scalars_is_one_bullet_joined_with_commas() {
    assert_eq!(
        render("S", json!({ "labels": ["bug", "ui"] })),
        "**S**\n\n- labels: bug, ui"
    );
}

#[test]
fn a_list_of_rows_is_a_heading_and_one_bullet_per_row_scalars_only() {
    assert_eq!(
        render(
            "2 branches.",
            json!({ "items": [
                { "name": "main", "sha": "abc1234", "protected": true, "nested": { "x": 1 } },
                { "name": "dev", "sha": "def5678", "protected": false, "tags": ["a", "b"] },
            ]})
        ),
        "**2 branches.**\n\n**items**\n- name: main · sha: abc1234 · protected: true\n- name: dev · sha: def5678 · protected: false · tags: a, b"
    );
}

#[test]
fn a_long_or_multi_line_field_of_a_row_is_an_indented_block_under_its_bullet() {
    let long = "x".repeat(300);
    assert_eq!(
        render(
            "S",
            json!({ "comments": [{ "author": "ada", "body": "line one\nline two" }, { "author": "bob", "body": long }] })
        ),
        format!(
            "**S**\n\n**comments**\n- author: ada\n  body:\n  ```text\n  line one\n  line two\n  ```\n- author: bob\n  body:\n  ```text\n  {long}\n  ```"
        )
    );
}

#[test]
fn a_short_field_of_a_row_stays_on_its_bullet_and_an_over_long_scalar_is_clipped() {
    assert_eq!(
        describe(&json!({ "a": "short", "b": ["x", "y"], "c": 3 })),
        "a: short · b: x, y · c: 3"
    );
    assert_eq!(
        describe(&json!("z".repeat(250))),
        format!("{}…", "z".repeat(200))
    );
}

#[test]
fn text_is_fenced_as_a_block_after_the_bullets() {
    assert_eq!(
        render("Read it.", json!({ "path": "a.txt", "text": "hello\n" })),
        "**Read it.**\n\n- path: a.txt\n\n```text\nhello\n```"
    );
}

#[test]
fn a_string_with_a_line_break_or_too_long_for_a_bullet_is_a_block_under_its_own_name() {
    assert_eq!(
        render(
            "Read it.",
            json!({ "sha": "abc", "message": "Subject\n\nBody line", "note": "short" })
        ),
        "**Read it.**\n\n- sha: abc\n- note: short\n\n**message**\n```text\nSubject\n\nBody line\n```"
    );
    let long = "z".repeat(250);
    assert_eq!(
        render("S", json!({ "body": long })),
        format!("**S**\n\n**body**\n```text\n{long}\n```")
    );
}

#[test]
fn an_empty_text_field_adds_no_block() {
    assert_eq!(render("S", json!({ "text": "" })), "**S**");
}

#[test]
fn a_fence_is_longer_than_any_run_of_backticks_inside_the_block() {
    assert_eq!(fence("a ``` b"), "````text\na ``` b\n````");
    assert_eq!(fence("plain"), "```text\nplain\n```");
}

#[test]
fn describe_renders_a_row_on_one_line_and_a_scalar_as_itself() {
    assert_eq!(
        describe(&json!({ "a": 1, "b": "x", "d": null, "e": "" })),
        "a: 1 · b: x"
    );
    assert_eq!(describe(&json!("solo")), "solo");
    assert_eq!(describe(&Value::Null), "");
}

#[test]
fn the_rendered_text_is_cut_at_the_output_ceiling() {
    let text = render("S", json!({ "text": "y".repeat(MAX_OUTPUT * 2) }));
    assert!(text.chars().count() <= MAX_OUTPUT + 20);
    assert!(text.ends_with("... (truncated)"));
}

#[test]
fn short_first_line_head_and_tail() {
    assert_eq!(short(&json!("0123456789")), "0123456");
    assert_eq!(short(&Value::Null), "");
    assert_eq!(first_line(&json!("one\ntwo")), "one");
    assert_eq!(first_line(&Value::Null), "");
    assert_eq!(
        head("abcdef", 3),
        ("abc\n... (truncated)".to_string(), true)
    );
    assert_eq!(head("abc", 3), ("abc".to_string(), false));
    assert_eq!(
        tail("abcdef", 3),
        ("(truncated) ...\ndef".to_string(), true)
    );
    assert_eq!(tail("abc", 3), ("abc".to_string(), false));
}

#[test]
fn decode_text_reads_utf8_falls_back_to_windows_1252_and_keeps_the_byte_order_mark() {
    assert_eq!(decode_text("héllo".as_bytes()), "héllo");
    assert_eq!(decode_text(&[0x63, 0x61, 0x66, 0xe9]), "café");
    assert_eq!(decode_text(&[0xef, 0xbb, 0xbf, 0x61]), "\u{FEFF}a");
}

#[test]
fn cut_text_cuts_at_a_character_count_and_never_inside_a_character() {
    assert_eq!(cut_text("abcdef", 3), ("abc".to_string(), true));
    assert_eq!(cut_text("abc", 3), ("abc".to_string(), false));
    // Counted in characters, so an emoji is one: the cut never falls inside it.
    assert_eq!(cut_text("a😀b", 2), ("a😀".to_string(), true));
    assert_eq!(cut_text("a😀b", 3), ("a😀b".to_string(), false));
}

#[test]
fn looks_binary_is_true_for_a_nul_byte_early_on_and_false_for_text() {
    assert!(looks_binary(&[0x68, 0x00, 0x69]));
    assert!(!looks_binary(b"plain text"));
    let mut late = vec![0x61u8; 9000];
    late.push(0);
    assert!(!looks_binary(&late));
}
