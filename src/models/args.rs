//! Argument checks. Every tool validates its own arguments before it reads the token or sends anything,
//! so a bad call costs no request and the refusal names the argument.
//!
//! Arguments arrive as untyped JSON, whatever the schema says, so every check takes a `&Value`: `null`
//! when the argument is missing.
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde_json::Value;

use crate::utils::errors::Refused;
use crate::utils::js::is_int;

/// The largest whole number a JSON number holds exactly, `Number.MAX_SAFE_INTEGER`.
pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// The arguments of one call, as an object. A call with none is an empty object.
pub type Args = serde_json::Map<String, Value>;

static NULL: Value = Value::Null;

/// One argument, or `null` when it was not given.
pub fn arg<'a>(args: &'a Args, name: &str) -> &'a Value {
    args.get(name).unwrap_or(&NULL)
}

/// `typeof value`, for a refusal.
pub fn js_typeof(value: &Value) -> &'static str {
    match value {
        Value::Null => "undefined",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) | Value::Object(_) => "object",
    }
}

/// An optional string argument: trimmed, and `None` when absent or blank.
pub fn optional_text(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => {
            let trimmed = s.trim();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            })
        }
        other => Err(Refused(format!(
            "`{field}` must be a string, got {}.",
            js_typeof(other)
        ))),
    }
}

/// A required string argument: trimmed, and never blank.
pub fn required_text(value: &Value, field: &str) -> Result<String, Refused> {
    optional_text(value, field)?.ok_or_else(|| {
        Refused(format!(
            "`{field}` is required and must be a non-empty string."
        ))
    })
}

/// Text kept exactly as given - a file's content, a comment body - and only checked to be a string.
pub fn raw_text(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        other => Err(Refused(format!(
            "`{field}` must be a string, got {}.",
            js_typeof(other)
        ))),
    }
}

/// Required text kept exactly as given. It may not be blank unless `allow_empty`.
pub fn required_raw_text(value: &Value, field: &str, allow_empty: bool) -> Result<String, Refused> {
    let text = raw_text(value, field)?.ok_or_else(|| Refused(format!("`{field}` is required.")))?;
    if !allow_empty && text.trim().is_empty() {
        return Err(Refused(format!(
            "`{field}` is required and must not be blank."
        )));
    }
    Ok(text)
}

/// An optional boolean argument, with the value to use when it is absent.
pub fn optional_flag(value: &Value, field: &str, fallback: bool) -> Result<bool, Refused> {
    Ok(maybe_flag(value, field)?.unwrap_or(fallback))
}

/// An optional boolean with no default, so "not given" and "false" stay different.
pub fn maybe_flag(value: &Value, field: &str) -> Result<Option<bool>, Refused> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        other => Err(Refused(format!(
            "`{field}` must be true or false, got {}.",
            js_typeof(other)
        ))),
    }
}

/// The bounds of a whole-number argument.
#[derive(Debug, Clone, Copy)]
pub struct IntRange {
    pub min: i64,
    pub max: i64,
}

impl IntRange {
    pub const fn min(min: i64) -> Self {
        IntRange {
            min,
            max: MAX_SAFE_INTEGER,
        }
    }
    pub const fn between(min: i64, max: i64) -> Self {
        IntRange { min, max }
    }
}

/// `JSON.stringify(value)`, for a refusal.
fn stringify(value: &Value) -> String {
    match value {
        Value::Number(n) => crate::utils::js::js_number(n),
        other => other.to_string(),
    }
}

/// An optional whole number inside a range.
pub fn optional_int(value: &Value, field: &str, range: IntRange) -> Result<Option<i64>, Refused> {
    if value.is_null() {
        return Ok(None);
    }
    if !is_int(value) {
        return Err(Refused(format!(
            "`{field}` must be a whole number, got {}.",
            stringify(value)
        )));
    }
    let number = value.as_f64().unwrap_or(f64::NAN);
    if number < range.min as f64 || number > range.max as f64 {
        return Err(Refused(format!(
            "`{field}` must be between {} and {}, got {}.",
            range.min,
            range.max,
            stringify(value)
        )));
    }
    Ok(value.as_i64().or(Some(number as i64)))
}

/// A required whole number inside a range.
pub fn required_int(value: &Value, field: &str, range: IntRange) -> Result<i64, Refused> {
    optional_int(value, field, range)?
        .ok_or_else(|| Refused(format!("`{field}` is required and must be a whole number.")))
}

/// Whether `text` is a date or time JavaScript's `Date.parse` reads in ISO 8601 form.
pub fn parses_as_date(text: &str) -> bool {
    if DateTime::parse_from_rfc3339(text).is_ok()
        || NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok()
    {
        return true;
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"] {
        if NaiveDateTime::parse_from_str(text, format).is_ok() {
            return true;
        }
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
        "%Y-%m-%dT%H:%M%:z",
        "%Y-%m-%dT%H:%MZ",
    ] {
        if DateTime::parse_from_str(text, format).is_ok() {
            return true;
        }
    }
    false
}

/// An optional date or timestamp, in ISO 8601 form such as `2026-10-01` or `2026-10-01T12:00:00Z`.
pub fn optional_date(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    let Some(text) = optional_text(value, field)? else {
        return Ok(None);
    };
    let shaped = text.len() >= 10 && {
        let b = text.as_bytes();
        b[..4].iter().all(u8::is_ascii_digit)
            && b[4] == b'-'
            && b[5..7].iter().all(u8::is_ascii_digit)
            && b[7] == b'-'
            && b[8..10].iter().all(u8::is_ascii_digit)
    };
    if !shaped || !parses_as_date(&text) {
        return Err(Refused(format!(
            "`{field}` must be an ISO 8601 date such as 2026-10-01 or 2026-10-01T12:00:00Z, got '{text}'."
        )));
    }
    Ok(Some(text))
}

/// One of a fixed set of words, with the value to use when it is absent.
pub fn one_of(
    value: &Value,
    field: &str,
    allowed: &[&str],
    fallback: Option<&str>,
) -> Result<Option<String>, Refused> {
    let Some(text) = optional_text(value, field)? else {
        return Ok(fallback.map(str::to_string));
    };
    if !allowed.contains(&text.as_str()) {
        return Err(Refused(format!(
            "`{field}` must be one of {}, got '{text}'.",
            allowed.join(", ")
        )));
    }
    Ok(Some(text))
}

/// The options of a list argument.
#[derive(Debug, Clone, Copy)]
pub struct ListOptions {
    pub required: bool,
    pub max: usize,
}

impl Default for ListOptions {
    fn default() -> Self {
        ListOptions {
            required: false,
            max: 100,
        }
    }
}

/// A list of non-blank strings, trimmed and without duplicates, in the order first given.
pub fn text_list(
    value: &Value,
    field: &str,
    options: ListOptions,
) -> Result<Option<Vec<String>>, Refused> {
    let entries = match value {
        Value::Null => {
            if options.required {
                return Err(Refused(format!(
                    "`{field}` is required and must list at least one value."
                )));
            }
            return Ok(None);
        }
        Value::Array(entries) => entries,
        _ => return Err(Refused(format!("`{field}` must be a list of strings."))),
    };
    let mut seen: Vec<String> = Vec::new();
    for entry in entries {
        let Value::String(entry) = entry else {
            return Err(Refused(format!("`{field}` must be a list of strings.")));
        };
        let text = entry.trim();
        if !text.is_empty() && !seen.iter().any(|s| s == text) {
            seen.push(text.to_string());
        }
    }
    if seen.len() > options.max {
        return Err(Refused(format!(
            "`{field}` may hold at most {} values, got {}.",
            options.max,
            seen.len()
        )));
    }
    if options.required && seen.is_empty() {
        return Err(Refused(format!(
            "`{field}` is required and must list at least one value."
        )));
    }
    Ok(Some(seen))
}

/// A list of label names. GitLab takes labels as one comma-separated string, so a name with a comma in it
/// cannot be sent and is refused here instead of being split into two labels.
pub fn label_list(
    value: &Value,
    field: &str,
    options: ListOptions,
) -> Result<Option<Vec<String>>, Refused> {
    let labels = text_list(value, field, options)?;
    if let Some(bad) = labels.iter().flatten().find(|label| label.contains(',')) {
        return Err(Refused(format!(
            "`{field}` may not contain a comma, because GitLab separates label names with one: '{bad}'."
        )));
    }
    Ok(labels)
}

/// `Date.parse(text)` for the ISO 8601 forms GitLab writes, in milliseconds; `None` where it is `NaN`.
/// A time without a zone is read as UTC.
pub fn date_millis(text: &str) -> Option<i64> {
    if let Ok(time) = DateTime::parse_from_rfc3339(text) {
        return Some(time.timestamp_millis());
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f%:z", "%Y-%m-%dT%H:%M:%S%.f%z"] {
        if let Ok(time) = DateTime::parse_from_str(text, format) {
            return Some(time.timestamp_millis());
        }
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"] {
        if let Ok(time) = NaiveDateTime::parse_from_str(text, format) {
            return Some(time.and_utc().timestamp_millis());
        }
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc().timestamp_millis())
}
