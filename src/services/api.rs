//! The GitLab client: the one place a GitLab URL is built and a call is made.
//!
//! Every tool reaches GitLab through `gitlab()`, and `branch_or_default`, which needs a call, and nothing
//! else builds a URL, so nothing else can let an argument steer a call at a different endpoint. The parts
//! of a path come only from `src/models/address.rs`, which validates and percent-encodes them; the request
//! itself is `src/services/client.rs`, and a refusal's wording is `src/services/failure.rs`.
use serde_json::Value;

use super::auth::require_token;
use super::client::{ApiAnswer, CallOptions, call_api};
use super::context::Ctx;
use super::failure::{Explain, FailedCall, failure};
use crate::config::config::API_BASE;
use crate::config::version::VERSION;
use crate::models::address::Repo;
use crate::utils::errors::Refused;

const JSON_ACCEPT: &str = "application/json";

/// How one call to GitLab is made, and how its failure is worded.
pub struct Call<'a> {
    query: Vec<(String, Option<String>)>,
    json: Option<Value>,
    accept: &'a str,
    allow: &'a [u16],
    what: String,
    explain: Option<Box<Explain<'a>>>,
    raw: bool,
}

impl<'a> Call<'a> {
    /// `what` completes "... to <what>" in a refusal.
    pub fn new(what: impl Into<String>) -> Self {
        Call {
            query: Vec::new(),
            json: None,
            accept: JSON_ACCEPT,
            allow: &[],
            what: what.into(),
            explain: None,
            raw: false,
        }
    }

    /// A query parameter; one that is absent or empty is left out.
    pub fn query(mut self, key: &str, value: Option<impl ToString>) -> Self {
        self.query
            .push((key.to_string(), value.map(|v| v.to_string())));
        self
    }

    /// Several query parameters at once.
    pub fn queries<K: ToString>(
        mut self,
        pairs: impl IntoIterator<Item = (K, Option<String>)>,
    ) -> Self {
        for (key, value) in pairs {
            self.query.push((key.to_string(), value));
        }
        self
    }

    pub fn json(mut self, json: Value) -> Self {
        self.json = Some(json);
        self
    }

    pub fn json_opt(mut self, json: Option<Value>) -> Self {
        self.json = json;
        self
    }

    /// Statuses that are an answer here and not a failure - a tag that does not exist, for one.
    pub fn allow(mut self, statuses: &'a [u16]) -> Self {
        self.allow = statuses;
        self
    }

    pub fn explain(
        mut self,
        explain: impl Fn(u16, &str) -> Option<String> + Send + Sync + 'a,
    ) -> Self {
        self.explain = Some(Box::new(explain));
        self
    }

    /// Keep a successful answer's body as text.
    /// Ask for another media type than JSON.
    pub fn accept(mut self, accept: &'a str) -> Self {
        self.accept = accept;
        self
    }

    pub fn raw(mut self) -> Self {
        self.raw = true;
        self
    }
}

/// The branch a call works on: the one asked for, or else the project's default branch, which a few
/// endpoints will not guess.
pub async fn branch_or_default(
    ctx: &Ctx,
    repo: &Repo,
    branch: Option<String>,
) -> Result<String, Refused> {
    if let Some(branch) = branch {
        return Ok(branch);
    }
    let answer = gitlab(
        ctx,
        "GET",
        &repo.path,
        Call::new(format!("find the default branch of {}", repo.full)),
    )
    .await?;
    match answer.body.get("default_branch").and_then(Value::as_str) {
        Some(branch) if !branch.is_empty() => Ok(branch.to_string()),
        _ => Err(Refused(format!(
            "{} has no default branch yet. Pass `branch`, or create the first commit another way.",
            repo.full
        ))),
    }
}

/// Call GitLab and return the answer, or refuse with a message that names the next step.
///
/// A 2xx answer is returned. So is any status listed in `allow`. Everything else is refused.
///
/// The token goes in an `Authorization: Bearer` header, which GitLab accepts for personal, project and
/// group access tokens. The client drops that header, and no other, when a redirect changes host;
/// GitLab's own `PRIVATE-TOKEN` header would be forwarded.
///
/// `path` starts with `/` and is built only from `parse_repo`, `ref_path`, `file_path` and numbers, so it
/// needs no further encoding. It carries no query string: use `Call::query`.
pub async fn gitlab(
    ctx: &Ctx,
    method: &str,
    path: &str,
    call: Call<'_>,
) -> Result<ApiAnswer, Refused> {
    assert!(
        path.starts_with('/') && !path.contains('?') && !path.contains('#'),
        "gitlab(): path must start with '/' and carry no query or fragment, got '{path}'."
    );

    let token = require_token(ctx)?;
    let mut url = reqwest::Url::parse(&format!("{API_BASE}{path}"))
        .expect("a URL built from validated parts");
    let pairs: Vec<(String, String)> = {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (key, value) in call.query {
            let Some(value) = value.filter(|v| !v.is_empty()) else {
                continue;
            };
            // `URLSearchParams.set`: a key given twice keeps its first place and its last value.
            match pairs.iter_mut().find(|(k, _)| *k == key) {
                Some(pair) => pair.1 = value,
                None => pairs.push((key, value)),
            }
        }
        pairs
    };
    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs.iter());
    }

    let answer = call_api(
        ctx.http.as_ref(),
        url.as_str(),
        CallOptions {
            method: method.to_string(),
            headers: vec![
                ("Authorization".into(), format!("Bearer {token}")),
                ("Accept".into(), call.accept.to_string()),
                (
                    "User-Agent".into(),
                    format!("lxagents-mcp-gitlab/{VERSION}"),
                ),
            ],
            json: call.json,
            token: token.clone(),
            raw: call.raw,
        },
    )
    .await?;

    if (200..300).contains(&answer.status) || call.allow.contains(&answer.status) {
        return Ok(answer);
    }
    Err(failure(FailedCall {
        status: answer.status,
        body: &answer.body,
        headers: &answer.headers,
        what: &call.what,
        explain: call.explain.as_deref(),
        token: &token,
    }))
}
