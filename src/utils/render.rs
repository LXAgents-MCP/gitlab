//! How a payload becomes the text the model reads.
//!
//! Every tool returns `{ summary, ...fields }`. The summary is the answer in a sentence; the fields are
//! what a client reads programmatically, and the text shows them too, because the text is what the model
//! actually sees:
//!
//! - a scalar field is a bullet, `- key: value`;
//! - a list of scalars is one bullet, joined with commas;
//! - a list of rows (`items`, `files`, `commits`, `comments`) is a heading and one bullet per row, and a
//!   long or multi-line field of a row (a comment's body) is a block indented under its bullet;
//! - `text` is a block - a file, a log, a diff - fenced so it cannot be mistaken for prose;
//! - any other string with a line break, or too long for a bullet, is a block under its own name, so a
//!   commit message or an issue body keeps its lines;
//! - a nested object is left to the structured content.
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::config::config::MAX_OUTPUT;
use crate::utils::js::js_string;
use crate::utils::text::{char_len, take_chars};

const CELL_MAX: usize = 200;

static LINE_BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\n\s*").expect("pattern"));
static TICKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`+").expect("pattern"));

fn is_scalar(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

/// A string that would not fit a bullet: it has a line break, or it is longer than a cell.
fn is_block_string(value: &Value) -> bool {
    matches!(value, Value::String(s) if s.contains('\n') || char_len(s) > CELL_MAX)
}

fn join_scalars(items: &[Value]) -> String {
    items.iter().map(js_string).collect::<Vec<_>>().join(", ")
}

/// One value of a row, on one line.
fn cell(value: &Value) -> String {
    match value {
        Value::Null | Value::Object(_) => String::new(),
        Value::Array(items) => {
            if items.iter().all(is_scalar) {
                join_scalars(items)
            } else {
                String::new()
            }
        }
        scalar => {
            let line = LINE_BREAK.replace_all(&js_string(scalar), " ").into_owned();
            if char_len(&line) > CELL_MAX {
                format!("{}…", take_chars(&line, CELL_MAX))
            } else {
                line
            }
        }
    }
}

/// The entries of an object, or of an array by index, as JavaScript's `Object.entries` lists them.
fn entries(row: &Value) -> Vec<(String, &Value)> {
    match row {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v)).collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => Vec::new(),
    }
}

/// A row as one line: `key: value · key: value`, scalars only.
pub fn describe(row: &Value) -> String {
    if is_scalar(row) {
        return cell(row);
    }
    entries(row)
        .into_iter()
        .filter(|(_, value)| !is_block_string(value))
        .map(|(key, value)| (key, cell(value)))
        .filter(|(_, text)| !text.is_empty())
        .map(|(key, text)| format!("{key}: {text}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// One row of a list: a bullet for its short fields, then each long field as an indented block.
fn render_row(row: &Value) -> String {
    let mut lines = vec![format!("- {}", describe(row))];
    for (key, value) in entries(row) {
        if let Value::String(text) = value
            && is_block_string(value)
        {
            for line in format!("{key}:\n{}", fence(text)).split('\n') {
                lines.push(format!("  {line}"));
            }
        }
    }
    lines.join("\n")
}

/// Fence a block with more backticks than any run inside it, so the content cannot close the fence.
pub fn fence(text: &str) -> String {
    let longest = TICKS
        .find_iter(text)
        .map(|m| m.as_str().len())
        .max()
        .unwrap_or(0);
    let ticks = "`".repeat(3.max(longest + 1));
    let body = text.strip_suffix('\n').unwrap_or(text);
    format!("{ticks}text\n{body}\n{ticks}")
}

/// The text of a payload: the summary in bold, then its fields as bullets, lists and blocks.
pub fn render_payload(summary: &str, fields: &Map<String, Value>) -> String {
    let mut parts: Vec<String> = vec![format!("**{summary}**")];
    let mut bullets: Vec<String> = Vec::new();
    let mut lists: Vec<(&str, &Vec<Value>)> = Vec::new();
    let mut blocks: Vec<(Option<&str>, &str)> = Vec::new();

    for (key, value) in fields {
        match value {
            Value::Null => {}
            Value::String(text) if key == "text" => {
                if !text.is_empty() {
                    blocks.push((None, text));
                }
            }
            Value::String(text) if is_block_string(value) => blocks.push((Some(key), text)),
            Value::Array(items) => {
                if items.is_empty() {
                    continue;
                }
                if items.iter().all(is_scalar) {
                    bullets.push(format!("- {key}: {}", join_scalars(items)));
                } else {
                    lists.push((key, items));
                }
            }
            scalar if is_scalar(scalar) => bullets.push(format!("- {key}: {}", cell(scalar))),
            _ => {}
        }
    }

    if !bullets.is_empty() {
        parts.push(String::new());
        parts.push(bullets.join("\n"));
    }
    for (key, rows) in lists {
        parts.push(String::new());
        parts.push(format!("**{key}**"));
        parts.push(rows.iter().map(render_row).collect::<Vec<_>>().join("\n"));
    }
    for (label, text) in blocks {
        parts.push(String::new());
        if let Some(label) = label {
            parts.push(format!("**{label}**"));
        }
        parts.push(fence(text));
    }

    let text = parts.join("\n");
    if char_len(&text) > MAX_OUTPUT {
        format!("{}\n... (truncated)", take_chars(&text, MAX_OUTPUT))
    } else {
        text
    }
}
