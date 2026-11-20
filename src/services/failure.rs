//! The one place a status GitLab answered with becomes a message that names the next step.
use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::config::config::TOKEN_ENV;
use crate::utils::errors::Refused;
use crate::utils::js::{js_string, truthy};
use crate::utils::redact::redact;

static WEB_PAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*<(!doctype|html)").expect("pattern"));

/// `[].concat(value).join(", ")`.
fn concat_join(value: &Value) -> String {
    match value {
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
            .join(", "),
        Value::Null => String::new(),
        other => js_string(other),
    }
}

/// What GitLab said went wrong, as a short suffix: ": message (detail; detail)".
///
/// GitLab answers in four shapes: a plain string (`{"message": "404 Project Not Found"}`), an object of
/// field errors (`{"message": {"name": ["has already been taken"]}}`), a list of messages, and an
/// OAuth-style `error` pair.
pub fn api_detail(body: &Value) -> String {
    match body {
        Value::String(text) => {
            // A security check or an outage answers with a page of HTML, which says nothing a reader can use.
            if WEB_PAGE.is_match(text) {
                return ": the answer was a web page and not the API's, such as a security check or an outage".into();
            }
            if text.is_empty() {
                String::new()
            } else {
                format!(": {text}")
            }
        }
        Value::Object(map) => {
            let message = map.get("message").unwrap_or(&Value::Null);
            match message {
                Value::String(text) => return format!(": {text}"),
                Value::Array(items) => {
                    return if items.is_empty() {
                        String::new()
                    } else {
                        format!(
                            ": {}",
                            items
                                .iter()
                                .map(|item| if item.is_null() {
                                    String::new()
                                } else {
                                    js_string(item)
                                })
                                .collect::<Vec<_>>()
                                .join("; ")
                        )
                    };
                }
                Value::Object(fields) => {
                    let parts: Vec<String> = fields
                        .iter()
                        .map(|(field, problems)| format!("{field} {}", concat_join(problems)))
                        .collect();
                    return if parts.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", parts.join("; "))
                    };
                }
                _ => {}
            }
            if let Some(Value::String(error)) = map.get("error") {
                let description = map.get("error_description").unwrap_or(&Value::Null);
                return if truthy(description) {
                    format!(": {error} ({})", js_string(description))
                } else {
                    format!(": {error}")
                };
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// `Number(text)` for a header value: blank is 0, anything that is not a number is not finite.
fn header_number(headers: &BTreeMap<String, String>, name: &str) -> f64 {
    match headers.get(name) {
        None => f64::NAN,
        Some(text) if text.trim().is_empty() => 0.0,
        Some(text) => text.trim().parse::<f64>().unwrap_or(f64::NAN),
    }
}

/// When the limit lifts, in words the model can act on.
fn when_to_retry(headers: &BTreeMap<String, String>) -> String {
    let seconds = header_number(headers, "retry-after");
    if seconds.is_finite() && seconds > 0.0 {
        return format!("Wait {} seconds and try again.", seconds.ceil() as i64);
    }
    let reset = header_number(headers, "ratelimit-reset");
    if reset.is_finite() && reset > 0.0 {
        let now = chrono::Utc::now().timestamp_millis() as f64;
        let minutes = (((reset * 1000.0 - now) / 60_000.0).ceil() as i64).max(1);
        let at = chrono::DateTime::from_timestamp_millis((reset * 1000.0) as i64)
            .map(|time| time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_default();
        return format!(
            "The limit resets at {at}, in about {minutes} minute{}.",
            if minutes == 1 { "" } else { "s" }
        );
    }
    "Wait a minute and try again.".into()
}

/// How a tool words the statuses it knows better than the general case: given the status and GitLab's own
/// detail suffix, a message, or `None` to fall through to the general wording.
pub type Explain<'a> = dyn Fn(u16, &str) -> Option<String> + Send + Sync + 'a;

/// An answer GitLab refused, and what the refusal needs to word it.
pub struct FailedCall<'a> {
    pub status: u16,
    pub body: &'a Value,
    pub headers: &'a BTreeMap<String, String>,
    pub what: &'a str,
    pub explain: Option<&'a Explain<'a>>,
    pub token: &'a str,
}

/// A refusal that says what to do next, for a status GitLab answered with.
pub fn failure(call: FailedCall<'_>) -> Refused {
    let FailedCall {
        status,
        body,
        headers,
        what,
        explain,
        token,
    } = call;
    let detail = api_detail(body);
    let message = explain.and_then(|explain| explain(status, &detail)).unwrap_or_else(|| match status {
        401 => format!("GitLab rejected the token (401). Check that {TOKEN_ENV} holds a valid token that has not expired."),
        429 => format!("GitLab is rate limiting this token (429){detail}. {}", when_to_retry(headers)),
        403 => format!(
            "GitLab refused to {what} (403){detail}. The token lacks permission for this: it needs the `api` \
             scope (`read_api` is enough only for reads), and its user needs a role in the project that allows it."
        ),
        404 => format!(
            "GitLab could not find what it needs to {what} (404){detail}. Check the spelling. GitLab also \
             answers 404 for a private project the token cannot see."
        ),
        405 => format!(
            "GitLab refused to {what} (405){detail}. The method is not allowed here; for a project that was \
             renamed or moved, use its current path or its numeric id."
        ),
        409 => format!("GitLab could not {what} (409){detail}. Something changed or conflicts; read it again before retrying."),
        400 | 422 => format!("GitLab could not {what} ({status}){detail}."),
        500.. => format!("GitLab failed to {what} ({status}){detail}. Try again shortly."),
        _ => format!("GitLab answered {status}{detail} when asked to {what}."),
    });
    Refused(redact(&message, token))
}
