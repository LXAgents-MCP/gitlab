//! Everything a tool reads from outside its arguments, in one value: the environment, the network and the
//! clock.
//!
//! Production reads the process environment when a tool is called (so a server without the GitLab key
//! still starts, and a key set later is seen), calls GitLab over HTTPS, and sleeps for real. A test hands
//! in a map, a queue of canned answers and a clock that does not wait, which is what lets tests run side
//! by side without sharing the process environment or a global HTTP client.
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;

/// Where a variable is read from.
pub trait Env: Send + Sync {
    fn var(&self, name: &str) -> Option<String>;
}

/// The process environment, read at the moment of asking.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnv;

impl Env for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// One HTTP request, as the client sends it.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    /// In the order and the case they were written.
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl HttpRequest {
    /// A header by name, ignoring case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// One HTTP answer: its status, its headers with lowercased names, and its body.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// Why no answer came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The call took longer than it may.
    Timeout,
    /// The answer was larger than the client will read.
    TooLarge,
    /// Anything else: no network, a refused connection, a TLS failure. The text says what.
    Failed(String),
}

/// Something that sends one HTTP request and reads at most `max_bytes` of the answer.
pub trait Backend: Send + Sync {
    fn send(
        &self,
        request: HttpRequest,
        max_bytes: usize,
    ) -> BoxFuture<'_, Result<HttpResponse, TransportError>>;
}

/// Something that waits.
pub trait Sleeper: Send + Sync {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()>;
}

/// The real clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioSleep;

impl Sleeper for TokioSleep {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        Box::pin(tokio::time::sleep(duration))
    }
}

/// The context every tool runs in.
#[derive(Clone)]
pub struct Ctx {
    pub env: Arc<dyn Env>,
    pub http: Arc<dyn Backend>,
    pub sleep: Arc<dyn Sleeper>,
}

impl Ctx {
    /// The context a real server runs in.
    pub fn production() -> Self {
        Ctx {
            env: Arc::new(ProcessEnv),
            http: Arc::new(crate::services::client::ReqwestBackend::new()),
            sleep: Arc::new(TokioSleep),
        }
    }

    /// A variable, trimmed, or the empty string.
    pub fn var(&self, name: &str) -> String {
        self.env.var(name).unwrap_or_default().trim().to_string()
    }
}
