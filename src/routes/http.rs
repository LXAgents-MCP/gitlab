//! The HTTP transport: `POST /mcp`, and `GET /healthz`.
//!
//! Two kinds of install, told apart by where the server listens:
//!
//! - **A server install** listens where other machines can reach it (`0.0.0.0`, or any `HOST` that is not
//!   loopback). Every route but `GET /healthz` needs `Authorization: Bearer <token>`, this server's own
//!   access token, `MCP_AUTH_TOKEN`. It is checked before anything else is read - before the body is read
//!   and before a path is matched - and it is never sent to GitLab. The server refuses to start without it.
//! - **A local install** listens on loopback only, and may leave `MCP_AUTH_TOKEN` unset. A web page open on
//!   this machine can still reach it through DNS rebinding, so without a token a request must say it is
//!   addressed to loopback in `Host`, and in `Origin` when it sends one. A token that is set is required
//!   here as well.
//!
//! The calls to GitLab are made with `GITLAB_API_KEY` from this process's environment, whoever the caller
//! is. Nothing is stored between requests: each one is served by a fresh MCP server, statelessly.
//! Diagnostics go to stderr, as everywhere in this server.
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{Method, Response, StatusCode, header};
use futures::FutureExt;
use http_body_util::{BodyExt, Limited};
use regex::Regex;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower::ServiceExt;

use crate::config::config::{
    AUTH_ENV, DEFAULT_HOST, DEFAULT_PORT, MAX_BODY_BYTES, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV,
};
use crate::config::http_settings::{HttpSettings, http_settings};
use crate::config::version::{SERVER_NAME, VERSION};
use crate::routes::server::{GitlabServer, Logger};
use crate::services::auth::{
    auth_token_problem, bearer_credential, configured_api_key, configured_auth_token,
    credential_matches, digest, is_loopback_host,
};
use crate::services::context::Ctx;
use crate::utils::http::{rpc_error, unauthorized};
use crate::utils::log::log;
use crate::utils::redact::redact;

static HOST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\[[0-9a-f:.]+\]|[a-z0-9.-]+)(?::\d{1,5})?$").expect("pattern")
});

/// Why a request to a local install - loopback, no access token - is refused, or `None` when it is not.
///
/// A browser always says where a request is going: `Host` is the name the page used, and `Origin` is the
/// page itself. A rebound name or another site shows up in one of them. `Host` is matched whole, as `name`
/// or `name:port`, so nothing in it can make a URL parser read a different host.
pub fn local_request_problem(host: Option<&str>, origin: Option<&str>) -> Option<&'static str> {
    let name = HOST
        .captures(host.unwrap_or(""))
        .map(|found| found[1].to_string());
    if !name.is_some_and(|name| is_loopback_host(&name)) {
        return Some(
            "Forbidden: this server has no access token, so it only answers requests addressed to localhost, 127.0.0.1 or [::1]",
        );
    }
    if let Some(origin) = origin
        && !origin_host(origin).is_some_and(|name| is_loopback_host(&name))
    {
        return Some(
            "Forbidden: this server has no access token, so it does not answer a web page from another origin",
        );
    }
    None
}

/// The host of an `http:` or `https:` origin, as a URL parser reads it.
fn origin_host(origin: &str) -> Option<String> {
    let url: axum::http::Uri = origin.trim().parse().ok()?;
    let scheme = url.scheme_str()?.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = url.authority()?;
    if authority.as_str().contains('@') {
        return None;
    }
    Some(authority.host().to_ascii_lowercase())
}

type McpService = StreamableHttpService<GitlabServer, NeverSessionManager>;

#[derive(Clone)]
struct AppState {
    expected: Option<[u8; 32]>,
    mcp: McpService,
}

/// A header's value, read byte for byte as Latin-1, as Node reads one: a credential with a byte outside
/// ASCII is still a credential, and a wrong one, not a missing one.
fn header_text(request: &Request, name: header::HeaderName) -> Option<String> {
    request
        .headers()
        .get(name)
        .map(|value| value.as_bytes().iter().map(|&byte| byte as char).collect())
}

/// Answer one request.
async fn handle(state: AppState, request: Request) -> Response<Body> {
    let path = request.uri().path().to_string();

    // The one open route, and exact: the method and the path, nothing that merely starts with it.
    if request.method() == Method::GET && path == "/healthz" {
        let body = serde_json::json!({ "status": "ok", "server": SERVER_NAME, "version": VERSION })
            .to_string();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, body.len())
            .body(Body::from(body))
            .expect("response");
    }

    match &state.expected {
        None => {
            if let Some(problem) = local_request_problem(
                header_text(&request, header::HOST).as_deref(),
                header_text(&request, header::ORIGIN).as_deref(),
            ) {
                return rpc_error(403, -32001, problem, &[]);
            }
        }
        Some(expected) => {
            match bearer_credential(header_text(&request, header::AUTHORIZATION).as_deref()) {
                None => return unauthorized(false),
                Some(credential) if !credential_matches(&credential, expected) => {
                    return unauthorized(true);
                }
                Some(_) => {}
            }
        }
    }

    if path != "/mcp" {
        return rpc_error(404, -32000, "Not found", &[]);
    }
    if request.method() != Method::POST {
        return rpc_error(
            405,
            -32000,
            "Method not allowed: use POST",
            &[("allow", "POST")],
        );
    }

    let (parts, body) = request.into_parts();
    let bytes: Bytes = match Limited::new(body, MAX_BODY_BYTES).collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) if error.is::<http_body_util::LengthLimitError>() => {
            return rpc_error(
                413,
                -32000,
                "Request body too large",
                &[("connection", "close")],
            );
        }
        Err(_) => return rpc_error(400, -32700, "Parse error: the body is not JSON", &[]),
    };
    if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
        return rpc_error(400, -32700, "Parse error: the body is not JSON", &[]);
    }

    let forwarded = Request::from_parts(parts, Body::from(bytes));
    match state.mcp.oneshot(forwarded).await {
        Ok(response) => response.map(Body::new),
        Err(never) => match never {},
    }
}

/// Where the HTTP transport listens, and the access token it asks for.
#[derive(Clone)]
pub struct HttpOptions {
    pub port: u16,
    pub host: String,
    /// The access token requests must carry; `None` reads `MCP_AUTH_TOKEN`.
    pub auth_token: Option<String>,
    pub ctx: Ctx,
    pub logger: Logger,
}

impl HttpOptions {
    pub fn new(ctx: Ctx) -> Self {
        HttpOptions {
            port: DEFAULT_PORT,
            host: DEFAULT_HOST.into(),
            auth_token: None,
            ctx,
            logger: Arc::new(log),
        }
    }
}

/// A listening HTTP transport.
pub struct RunningServer {
    pub port: u16,
    pub host: String,
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl RunningServer {
    /// Stop listening and wait, at most five seconds, for the requests in flight.
    pub async fn close(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
        }
    }
}

/// Start listening. Port 0 asks the system for a free one. The access token may be empty only when the
/// host is loopback, a local install; otherwise it must pass `auth_token_problem`, or nothing listens.
pub async fn start_http_server(options: HttpOptions) -> Result<RunningServer, String> {
    let token = options
        .auth_token
        .clone()
        .unwrap_or_else(|| configured_auth_token(&options.ctx));
    let local = token.is_empty() && is_loopback_host(&options.host);
    if !local && let Some(problem) = auth_token_problem(&token) {
        return Err(problem);
    }
    let expected = (!local).then(|| digest(&token));

    let ctx = options.ctx.clone();
    let logger = options.logger.clone();
    let factory_logger = logger.clone();
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        // The guard above already decided who may call; the SDK's own host check would refuse a server install.
        .disable_allowed_hosts()
        .with_max_request_body_bytes(MAX_BODY_BYTES);
    let mcp = StreamableHttpService::new(
        move || {
            let mut server = GitlabServer::new(ctx.clone());
            server.logger = factory_logger.clone();
            Ok(server)
        },
        Arc::new(NeverSessionManager::default()),
        config,
    );
    let state = AppState { expected, mcp };

    let crash_ctx = options.ctx.clone();
    let app = Router::new().fallback(move |request: Request| {
        let state = state.clone();
        let logger = logger.clone();
        let ctx = crash_ctx.clone();
        let token = token.clone();
        async move {
            let presented =
                bearer_credential(header_text(&request, header::AUTHORIZATION).as_deref())
                    .unwrap_or_default();
            match std::panic::AssertUnwindSafe(handle(state, request))
                .catch_unwind()
                .await
            {
                Ok(response) => response,
                Err(panic) => {
                    // A crash is text nobody wrote for a reader, so it is scrubbed of every secret: the
                    // GitLab key, the access token, and whatever credential this request presented.
                    let text = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    let text = format!("HTTP handler crashed:\n{text}");
                    logger(&redact(
                        &redact(&redact(&text, &configured_api_key(&ctx)), &token),
                        &presented,
                    ));
                    rpc_error(500, -32603, "Internal error", &[])
                }
            }
        }
    });

    let host = options.host.clone();
    let address = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let listener = TcpListener::bind(format!("{address}:{}", options.port))
        .await
        .map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map(|a: SocketAddr| a.port())
        .unwrap_or(options.port);
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = stopped.await;
            })
            .await;
    });
    Ok(RunningServer {
        port,
        host,
        stop: Some(stop),
        task,
    })
}

/// Run the HTTP transport as this process's job: read the settings, listen, and close on a signal. The
/// answer is the exit code.
pub async fn run_http(ctx: Ctx) -> i32 {
    let env_ctx = ctx.clone();
    let settings: HttpSettings = match http_settings(&|name| Some(env_ctx.var(name))) {
        Ok(settings) => settings,
        Err(message) => {
            log(&message);
            return 2;
        }
    };

    // A local install - loopback, and no token - needs none. Anything else fails closed, before anything
    // listens: HTTP without an access token where other machines can reach it is an open door.
    let local = configured_auth_token(&ctx).is_empty() && is_loopback_host(&settings.host);
    let problem = if local {
        None
    } else {
        auth_token_problem(&configured_auth_token(&ctx))
    };
    if let Some(problem) = problem {
        log(&format!(
            "{SERVER_NAME} {VERSION} will not start the HTTP transport on {}: {problem}. Other machines can reach \
             that address, so set {AUTH_ENV} to a secret of at least {MIN_AUTH_TOKEN_LENGTH} characters, for example \
             the output of: openssl rand -hex 32. A local install needs no access token: leave HOST unset, or set it \
             to 127.0.0.1, and leave {AUTH_ENV} unset. stdio needs no access token.",
            settings.host
        ));
        return 1;
    }

    // Not fatal: the server starts and a tool names the variable when it is called. Said at start as well,
    // because over HTTP nobody is watching the first call.
    if configured_api_key(&ctx).is_empty() {
        log(&format!(
            "{TOKEN_ENV} is not set in this process's environment: every tool will refuse until it is."
        ));
    }

    let mut options = HttpOptions::new(ctx);
    options.port = settings.port;
    options.host = settings.host.clone();
    let running = match start_http_server(options).await {
        Ok(running) => running,
        Err(message) => {
            log(&format!(
                "{SERVER_NAME} {VERSION} could not listen on {}:{}: {message}",
                settings.host, settings.port
            ));
            return 1;
        }
    };

    // Installed before the line that says the server is up, so a signal sent on seeing it is handled.
    let first_signal = shutdown_signal();
    let name = if running.host.contains(':') {
        format!("[{}]", running.host)
    } else {
        running.host.clone()
    };
    let place = if running.host == "0.0.0.0" {
        format!(":{}/mcp (all interfaces)", running.port)
    } else {
        format!("http://{name}:{}/mcp", running.port)
    };
    log(&format!(
        "{SERVER_NAME} {VERSION} serving over http on {place}"
    ));
    if local {
        log(&format!(
            "local install: {AUTH_ENV} is not set, so only this machine can reach the server and no access token \
             is asked for. Requests must be addressed to localhost, 127.0.0.1 or [::1]. To serve other machines, \
             set {AUTH_ENV} and leave HOST unset or make it 0.0.0.0."
        ));
    } else {
        log(&format!(
            "the access token in {AUTH_ENV} is required on every route except GET /healthz."
        ));
    }

    first_signal.await;
    // A second signal means the operator has stopped waiting.
    tokio::select! {
        _ = running.close() => {}
        _ = shutdown_signal() => {}
    }
    0
}

/// SIGINT or SIGTERM. In a container this process is PID 1, which gets no default action for a signal,
/// so without a handler `docker stop` waits out its grace period and then kills it.
pub fn shutdown_signal() -> impl std::future::Future<Output = ()> + Send + 'static {
    // The handlers are installed here, when this is called, not when the future is first polled: a
    // signal that arrives between the two must not fall through to the default action.
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt = signal(SignalKind::interrupt()).expect("signal handler");
        let mut terminate = signal(SignalKind::terminate()).expect("signal handler");
        async move {
            tokio::select! {
                _ = interrupt.recv() => {}
                _ = terminate.recv() => {}
            }
        }
    }
    #[cfg(not(unix))]
    {
        async {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}
