//! The type of an answer from GitLab, and how a tool reads one: JavaScript's semantics for JSON.
//!
//! A body is JSON that came over the network, from an API that adds fields over time and from a proxy or
//! an outage that can send anything at all. A hand-written struct per endpoint would be a claim nothing
//! checks, so a body stays a loose `serde_json::Value` where it enters, and every tool checks the one
//! field it relies on (`expect_body` in `shapes.rs`) before it reads the rest. The helpers here read a
//! value the way the JavaScript this server was first written in did, so each tool still says what it
//! said: optional chaining, `??`, truthiness and `String()`.
use serde_json::Value;

static NULL: Value = Value::Null;

/// A field down a dotted path, or `null` when any step is missing or not an object: `a?.b?.c`.
pub fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    let mut current = value;
    for key in path.split('.') {
        current = match current {
            Value::Object(map) => map.get(key).unwrap_or(&NULL),
            _ => &NULL,
        };
    }
    current
}

/// `value ?? fallback`: `null` and missing count as absent.
pub fn or<'a>(value: &'a Value, fallback: &'a Value) -> &'a Value {
    if value.is_null() { fallback } else { value }
}

/// JavaScript's truthiness: false for `null`, `false`, `0`, `NaN` and `""`.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// The string, when the value is one.
pub fn as_str(value: &Value) -> Option<&str> {
    value.as_str()
}

/// A string that is there and not empty.
pub fn nonempty_str(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty())
}

/// `Number.isInteger(value)`.
pub fn is_int(value: &Value) -> bool {
    match value {
        Value::Number(n) => {
            n.is_i64()
                || n.is_u64()
                || n.as_f64()
                    .is_some_and(|f| f.is_finite() && f.fract() == 0.0)
        }
        _ => false,
    }
}

/// The value as a whole number, when it is one.
pub fn as_int(value: &Value) -> Option<i64> {
    if !is_int(value) {
        return None;
    }
    value.as_i64().or_else(|| value.as_f64().map(|f| f as i64))
}

/// A JavaScript number as `String()` writes it: `7`, not `7.0`.
pub fn js_number(n: &serde_json::Number) -> String {
    if n.is_i64() || n.is_u64() {
        return n.to_string();
    }
    match n.as_f64() {
        Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e21 => format!("{}", f as i64),
        Some(f) => format!("{f}"),
        None => n.to_string(),
    }
}

/// `String(value)`, for the scalars a tool puts into a sentence. A missing field is "undefined", as it
/// was in JavaScript; no message relies on it.
pub fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "undefined".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => js_number(n),
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                if item.is_null() {
                    String::new()
                } else {
                    js_string(item)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// The value as a list: the array, or nothing when it is not one (`Array.isArray(x) ? x : []`).
pub fn list(value: &Value) -> &[Value] {
    match value {
        Value::Array(items) => items,
        _ => &[],
    }
}

/// JavaScript's `Number.parseInt(text, 10)`: the leading whole number, if there is one.
pub fn parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start();
    let (sign, rest) = match text.as_bytes().first() {
        Some(b'-') => (-1, &text[1..]),
        Some(b'+') => (1, &text[1..]),
        _ => (1, text),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<i64>().ok().map(|n| sign * n)
}

/// The keys of an object in the order JavaScript's `Object.entries` gives them: whole-number keys first,
/// ascending, then the rest in the order they were written.
pub fn js_entries(object: &serde_json::Map<String, Value>) -> Vec<(&String, &Value)> {
    let index = |key: &str| -> Option<u32> {
        let canonical = key == "0"
            || (!key.is_empty()
                && !key.starts_with('0')
                && key.bytes().all(|b| b.is_ascii_digit()));
        if canonical {
            key.parse::<u32>().ok().filter(|n| *n != u32::MAX)
        } else {
            None
        }
    };
    let mut numeric: Vec<(u32, (&String, &Value))> = object
        .iter()
        .filter_map(|(k, v)| index(k).map(|n| (n, (k, v))))
        .collect();
    numeric.sort_by_key(|(n, _)| *n);
    let mut entries: Vec<(&String, &Value)> = numeric.into_iter().map(|(_, entry)| entry).collect();
    entries.extend(object.iter().filter(|(k, _)| index(k).is_none()));
    entries
}
