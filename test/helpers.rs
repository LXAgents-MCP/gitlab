//! Shared test support: a context with a map for an environment, a queue of canned GitLab answers that
//! records every request, and a clock that does not wait.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::BoxFuture;
use lxagents_gitlab::config::config::TOKEN_ENV;
use lxagents_gitlab::controllers::get;
use lxagents_gitlab::models::outcome::Outcome;
use lxagents_gitlab::services::context::{
    Backend, Ctx, Env, HttpRequest, HttpResponse, Sleeper, TransportError,
};
use regex::Regex;
use serde_json::{Map, Value, json};

pub const TOKEN: &str = "glpat-test-token-1234";
pub const API: &str = "https://gitlab.com/api/v4";

/* ------------------------------------------------------------------ environment */

#[derive(Default)]
pub struct MapEnv(pub Mutex<HashMap<String, String>>);

impl Env for MapEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }
}

/* ------------------------------------------------------------------ the GitLab API */

/// One canned answer.
#[derive(Clone, Debug)]
pub struct Reply {
    pub status: u16,
    pub body: Option<String>,
    pub headers: Vec<(String, String)>,
    pub fail: Option<TransportError>,
}

/// An answer with a JSON body; `Value::Null` sends no body, a string is sent as it is.
pub fn reply(status: u16, body: Value) -> Reply {
    let body = match body {
        Value::Null => None,
        Value::String(text) => Some(text),
        other => Some(other.to_string()),
    };
    Reply {
        status,
        body,
        headers: Vec::new(),
        fail: None,
    }
}

/// An answer with headers.
pub fn reply_with(status: u16, body: Value, headers: &[(&str, &str)]) -> Reply {
    let mut answer = reply(status, body);
    answer.headers = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    answer
}

/// No answer at all.
pub fn failure(error: TransportError) -> Reply {
    Reply {
        status: 0,
        body: None,
        headers: Vec::new(),
        fail: Some(error),
    }
}

/// A request the mock saw. `body` is the JSON sent, decoded.
#[derive(Clone, Debug)]
pub struct RecordedCall {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
    pub max_bytes: usize,
}

impl RecordedCall {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
    pub fn parsed(&self) -> reqwest::Url {
        reqwest::Url::parse(&self.url).expect("url")
    }
    /// The path, still percent-encoded.
    pub fn path(&self) -> String {
        self.parsed().path().to_string()
    }
    /// The query, decoded, as `Object.fromEntries(url.searchParams)` reads it.
    pub fn query(&self) -> BTreeMap<String, String> {
        self.parsed()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }
    pub fn param(&self, name: &str) -> Option<String> {
        self.parsed()
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    }
    pub fn body(&self) -> &Value {
        self.body.as_ref().unwrap_or(&Value::Null)
    }
}

#[derive(Default)]
pub struct MockApi {
    pub queue: Mutex<Vec<Reply>>,
    pub calls: Mutex<Vec<RecordedCall>>,
}

impl Backend for MockApi {
    fn send(
        &self,
        request: HttpRequest,
        max_bytes: usize,
    ) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        let body = request
            .body
            .as_deref()
            .map(|b| serde_json::from_str(b).unwrap_or(Value::String(b.to_string())));
        let mut calls = self.calls.lock().unwrap();
        calls.push(RecordedCall {
            url: request.url.clone(),
            method: request.method.clone(),
            headers: request.headers.clone(),
            body,
            max_bytes,
        });
        let mut queue = self.queue.lock().unwrap();
        if queue.is_empty() {
            panic!(
                "unexpected request #{}: {} {}",
                calls.len(),
                request.method,
                request.url
            );
        }
        let next = queue.remove(0);
        Box::pin(async move {
            if let Some(error) = next.fail {
                return Err(error);
            }
            Ok(HttpResponse {
                status: next.status,
                headers: next
                    .headers
                    .into_iter()
                    .map(|(k, v)| (k.to_lowercase(), v))
                    .collect(),
                body: next.body.unwrap_or_default().into_bytes(),
            })
        })
    }
}

/* ------------------------------------------------------------------ the clock */

#[derive(Default)]
pub struct NoSleep(pub Mutex<Vec<Duration>>);

impl Sleeper for NoSleep {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        self.0.lock().unwrap().push(duration);
        Box::pin(async {})
    }
}

/* ------------------------------------------------------------------ the harness */

pub struct Harness {
    pub env: Arc<MapEnv>,
    pub api: Arc<MockApi>,
    pub sleeps: Arc<NoSleep>,
    pub ctx: Ctx,
}

impl Harness {
    /// A context with no variables set.
    pub fn new() -> Self {
        let env = Arc::new(MapEnv::default());
        let api = Arc::new(MockApi::default());
        let sleeps = Arc::new(NoSleep::default());
        let ctx = Ctx {
            env: env.clone(),
            http: api.clone(),
            sleep: sleeps.clone(),
        };
        Harness {
            env,
            api,
            sleeps,
            ctx,
        }
    }

    /// A context with the GitLab key set.
    pub fn with_token() -> Self {
        let harness = Self::new();
        harness.set(TOKEN_ENV, TOKEN);
        harness
    }

    pub fn set(&self, name: &str, value: &str) {
        self.env
            .0
            .lock()
            .unwrap()
            .insert(name.to_string(), value.to_string());
    }

    pub fn unset(&self, name: &str) {
        self.env.0.lock().unwrap().remove(name);
    }

    /// Queue answers, one per request in order.
    pub fn mock(&self, answers: Vec<Reply>) {
        self.api.queue.lock().unwrap().extend(answers);
    }

    pub fn calls(&self) -> Vec<RecordedCall> {
        self.api.calls.lock().unwrap().clone()
    }

    pub fn call_count(&self) -> usize {
        self.api.calls.lock().unwrap().len()
    }

    /// Run a tool's handler.
    pub async fn run(&self, tool: &str, args: Value) -> Result<Outcome, String> {
        let spec = get(tool).unwrap_or_else(|| panic!("no tool {tool}"));
        let args = args.as_object().cloned().unwrap_or_default();
        (spec.handle)(&self.ctx, &args)
            .await
            .map_err(|refused| refused.0)
    }

    /// Run a tool that must succeed; its payload is checked against its output schema.
    pub async fn ok(&self, tool: &str, args: Value) -> Value {
        let outcome = self
            .run(tool, args)
            .await
            .unwrap_or_else(|why| panic!("{tool} refused: {why}"));
        let payload = payload(&outcome);
        expect_output(tool, &payload);
        payload
    }

    /// Run a tool and return its whole outcome, checked against its output schema.
    pub async fn outcome(&self, tool: &str, args: Value) -> Outcome {
        let outcome = self
            .run(tool, args)
            .await
            .unwrap_or_else(|why| panic!("{tool} refused: {why}"));
        expect_output(tool, &payload(&outcome));
        outcome
    }

    /// Run a tool that must refuse; the answer is its message.
    pub async fn refused(&self, tool: &str, args: Value) -> String {
        match self.run(tool, args).await {
            Ok(outcome) => panic!("{tool} did not refuse: {}", outcome.summary),
            Err(message) => message,
        }
    }
}

/// An outcome as the TypeScript tests read a payload: the summary first, then the fields.
pub fn payload(outcome: &Outcome) -> Value {
    let mut map = Map::new();
    map.insert("summary".into(), Value::String(outcome.summary.clone()));
    for (k, v) in &outcome.fields.0 {
        map.insert(k.clone(), v.clone());
    }
    Value::Object(map)
}

/// Assert that `text` matches the JavaScript-style pattern `pattern`.
#[track_caller]
pub fn assert_match(text: &str, pattern: &str) {
    let re = Regex::new(pattern).unwrap_or_else(|e| panic!("bad pattern {pattern}: {e}"));
    assert!(re.is_match(text), "{text:?}\n  does not match /{pattern}/");
}

#[track_caller]
pub fn assert_no_match(text: &str, pattern: &str) {
    let re = Regex::new(pattern).unwrap();
    assert!(!re.is_match(text), "{text:?}\n  matches /{pattern}/");
}

/* ------------------------------------------------------------------ output schemas */

/// A successful payload must satisfy the tool's own output schema - the check a client makes on
/// `structuredContent` - and lead with a summary sentence.
#[track_caller]
pub fn expect_output(tool: &str, payload: &Value) {
    let mut fields = payload.as_object().cloned().unwrap_or_default();
    let summary = fields.remove("summary");
    let text = summary
        .as_ref()
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{tool} has no summary"));
    assert!(
        !text.is_empty() && text == text.trim(),
        "{tool} summary is blank or padded"
    );
    let schema = &get(tool).expect("tool").def.output_schema;
    if let Err(problem) = validate(schema, &Value::Object(fields), "") {
        panic!("{tool} payload breaks its outputSchema: {problem}");
    }
}

/// The subset of JSON Schema the output schemas use.
pub fn validate(schema: &Value, value: &Value, at: &str) -> Result<(), String> {
    if let Some(kind) = schema.get("type") {
        let kinds: Vec<&str> = match kind {
            Value::String(k) => vec![k.as_str()],
            Value::Array(ks) => ks.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        let fits = kinds.iter().any(|k| match *k {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_f64().is_some_and(|n| n.fract() == 0.0),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => true,
        });
        if !fits {
            return Err(format!("{at}: expected {kinds:?}, got {value}"));
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(value)
    {
        return Err(format!("{at}: {value} is not one of {allowed:?}"));
    }
    if let Some(object) = value.as_object() {
        let props = schema.get("properties").and_then(Value::as_object);
        for name in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = name.as_str().unwrap();
            if !object.contains_key(name) {
                return Err(format!("{at}.{name}: required"));
            }
        }
        for (key, item) in object {
            match props.and_then(|p| p.get(key)) {
                Some(sub) => validate(sub, item, &format!("{at}.{key}"))?,
                None => {
                    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                        return Err(format!("{at}.{key}: not allowed"));
                    }
                }
            }
        }
    }
    if let (Some(items), Some(array)) = (schema.get("items"), value.as_array()) {
        for (i, item) in array.iter().enumerate() {
            validate(items, item, &format!("{at}[{i}]"))?;
        }
    }
    Ok(())
}

pub fn ok_json() -> Value {
    json!({})
}
