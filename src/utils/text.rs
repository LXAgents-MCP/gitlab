//! Small text helpers shared by the tools. Pure functions, no I/O.
//!
//! Lengths are counted in characters (Unicode scalar values), and every cut falls on a character
//! boundary, so a cut never leaves half a character behind.
use serde_json::Value;
use std::sync::LazyLock;

use regex::Regex;

/// The abbreviated form of a commit sha.
pub fn short(sha: &Value) -> String {
    match sha {
        Value::String(s) => s.chars().take(7).collect(),
        _ => String::new(),
    }
}

/// The abbreviated form of a commit sha given as text.
pub fn short_str(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The first line of a message, which is all a listing needs.
pub fn first_line(message: &Value) -> String {
    match message {
        Value::String(s) => s
            .split('\n')
            .next()
            .unwrap_or("")
            .trim_end_matches('\r')
            .to_string(),
        _ => String::new(),
    }
}

/// Decode bytes as text: UTF-8 when they are valid UTF-8, Windows-1252 when they are not. A byte order
/// mark is kept, so a caller that cares can strip it explicitly.
pub fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

/// The number of characters in `text`.
pub fn char_len(text: &str) -> usize {
    text.chars().count()
}

/// The first `max` characters of `text`.
pub fn take_chars(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((at, _)) => &text[..at],
        None => text,
    }
}

/// The last `max` characters of `text`.
pub fn last_chars(text: &str, max: usize) -> &str {
    let len = char_len(text);
    if len <= max {
        return text;
    }
    match text.char_indices().nth(len - max) {
        Some((at, _)) => &text[at..],
        None => "",
    }
}

/// Cut text to a number of characters. Returns the text and whether it was cut.
pub fn cut_text(text: &str, max: usize) -> (String, bool) {
    if char_len(text) <= max {
        (text.to_string(), false)
    } else {
        (take_chars(text, max).to_string(), true)
    }
}

/// Does this look like binary data? A NUL byte early on is the usual tell.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

/// Cut text to a length, keeping the start. Returns the text and whether it was cut.
pub fn head(text: &str, max: usize) -> (String, bool) {
    if char_len(text) > max {
        (format!("{}\n... (truncated)", take_chars(text, max)), true)
    } else {
        (text.to_string(), false)
    }
}

/// Cut text to a length, keeping the end - where a log says why it failed. Returns the text and whether
/// it was cut.
pub fn tail(text: &str, max: usize) -> (String, bool) {
    if char_len(text) > max {
        (format!("(truncated) ...\n{}", last_chars(text, max)), true)
    } else {
        (text.to_string(), false)
    }
}

static SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"section_(?:start|end):\d+:[^\r\n\x1b]*\r?").expect("pattern"));
static BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\r\n?").expect("pattern"));
static COLOUR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").expect("pattern"));

/// A job's log as text to read: one kind of line break, and none of the colour codes that tools write for
/// a terminal, or of the `section_start` and `section_end` markers GitLab's runner adds to fold a log in
/// its web page, which only add noise.
pub fn clean_log(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = SECTION.replace_all(text, "");
    let text = BREAK.replace_all(&text, "\n");
    COLOUR.replace_all(&text, "").into_owned()
}
