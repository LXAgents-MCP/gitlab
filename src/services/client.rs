//! The HTTP client under every GitLab call: one request with a timeout and a size cap, and every failure
//! to get an answer turned into a `Refused` the model can read.
use std::collections::BTreeMap;
use std::error::Error as _;
use std::time::Duration;

use futures::StreamExt;
use futures::future::BoxFuture;
use serde_json::Value;

use super::context::{Backend, HttpRequest, HttpResponse, TransportError};
use crate::config::config::{API_TIMEOUT_MS, MAX_RESPONSE_BYTES};
use crate::utils::errors::Refused;
use crate::utils::redact::redact;
use crate::utils::text::take_chars;

/// An answer from a hosting API: its status, its decoded body and its headers, lowercased.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiAnswer {
    pub status: u16,
    pub body: Value,
    pub headers: BTreeMap<String, String>,
}

/// How `call_api` makes its one request.
#[derive(Debug, Clone, Default)]
pub struct CallOptions {
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub json: Option<Value>,
    /// Only used to scrub error text; the caller puts the credential in `headers`.
    pub token: String,
    /// Keeps the whole body of a successful answer as text instead of decoding JSON; an error answer is
    /// still decoded, so that its message can be read.
    pub raw: bool,
}

/// `new URL(url).host`: the host, and the port when it is not the scheme's own.
fn host_of(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or("").to_string();
            match parsed.port() {
                Some(port) => format!("{host}:{port}"),
                None => host,
            }
        }
        Err(_) => url.to_string(),
    }
}

/// Call a hosting API and return its status, decoded body and headers.
///
/// Every failure to get an answer - no network, a refused connection, a timeout - becomes a `Refused` the
/// model can read and act on. A non-2xx answer is not a failure here: the caller knows what each status
/// means.
pub async fn call_api(
    backend: &dyn Backend,
    url: &str,
    options: CallOptions,
) -> Result<ApiAnswer, Refused> {
    let host = host_of(url);
    let mut headers = options.headers.clone();
    let body = options.json.as_ref().map(|json| {
        headers.push(("Content-Type".into(), "application/json".into()));
        json.to_string()
    });
    let method = if options.method.is_empty() {
        "GET".to_string()
    } else {
        options.method.clone()
    };
    let request = HttpRequest {
        method,
        url: url.to_string(),
        headers,
        body,
    };

    let response = match backend.send(request, MAX_RESPONSE_BYTES).await {
        Ok(response) => response,
        Err(TransportError::Timeout) => {
            return Err(Refused(format!(
                "{host} did not answer within {} seconds. Try again.",
                API_TIMEOUT_MS / 1000
            )));
        }
        Err(TransportError::TooLarge) => {
            return Err(Refused(format!(
                "The answer from {host} is larger than {} MB. Ask for less.",
                MAX_RESPONSE_BYTES / 1_000_000
            )));
        }
        Err(TransportError::Failed(reason)) => {
            return Err(Refused(format!(
                "Could not reach {host}: {}. Check the network connection.",
                redact(&reason, &options.token)
            )));
        }
    };

    let ok = (200..300).contains(&response.status);
    let text = String::from_utf8_lossy(&response.body).into_owned();
    let body = if text.is_empty() {
        Value::Null
    } else if options.raw && ok {
        Value::String(text)
    } else {
        serde_json::from_str(&text)
            .unwrap_or_else(|_| Value::String(take_chars(&text, 300).to_string()))
    };
    Ok(ApiAnswer {
        status: response.status,
        body,
        headers: response.headers,
    })
}

/// The real network: one shared client, rustls, a 30-second limit on the whole call, and at most ten
/// redirects. On a redirect that changes host or port the client drops `Authorization`, as `fetch` does,
/// so the token is never sent to a second host.
pub struct ReqwestBackend {
    client: reqwest::Client,
}

impl ReqwestBackend {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(API_TIMEOUT_MS))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("an HTTP client with the default TLS configuration");
        ReqwestBackend { client }
    }
}

impl Default for ReqwestBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// The innermost reason an error gives, as `error.cause.message ?? error.message` read it.
fn reason(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text = inner.to_string();
        source = inner.source();
    }
    text
}

fn transport_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else {
        TransportError::Failed(reason(&error))
    }
}

impl Backend for ReqwestBackend {
    fn send(
        &self,
        request: HttpRequest,
        max_bytes: usize,
    ) -> BoxFuture<'_, Result<HttpResponse, TransportError>> {
        Box::pin(async move {
            let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|_| {
                TransportError::Failed(format!("'{}' is not an HTTP method", request.method))
            })?;
            let mut builder = self.client.request(method, &request.url);
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            let response = builder.send().await.map_err(transport_error)?;
            let status = response.status().as_u16();
            let mut headers = BTreeMap::new();
            for (name, value) in response.headers() {
                let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
                headers
                    .entry(name.as_str().to_ascii_lowercase())
                    .and_modify(|joined: &mut String| {
                        joined.push_str(", ");
                        joined.push_str(&value);
                    })
                    .or_insert(value);
            }
            let mut body = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(transport_error)?;
                if body.len() + chunk.len() > max_bytes {
                    return Err(TransportError::TooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(HttpResponse {
                status,
                headers,
                body,
            })
        })
    }
}
