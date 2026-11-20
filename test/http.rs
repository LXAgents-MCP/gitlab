//! The HTTP transport over real sockets on loopback.
//!
//! The client here is a few lines of HTTP/1.1 over a TCP socket, so a test can send exactly the `Host`,
//! `Origin` and method it means to, with no proxy and no client defaults in the way.
//!
//! Two secrets are in play and the tests keep them apart: ACCESS is the server's own token, which a
//! client presents; TOKEN is the GitLab key, which the server holds and a client never sees.
use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lxagents_gitlab::config::config::{
    AUTH_ENV, DEFAULT_HOST, DEFAULT_PORT, LOCAL_HOST, MAX_BODY_BYTES, TOKEN_ENV,
};
use lxagents_gitlab::config::http_settings::{HttpSettings, http_settings};
use lxagents_gitlab::config::version::{SERVER_NAME, VERSION};
use lxagents_gitlab::routes::http::{
    HttpOptions, RunningServer, local_request_problem, start_http_server,
};
use lxagents_gitlab::routes::server::Logger;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::helpers::*;
use crate::surface::tool_names;

const BIN: &str = env!("CARGO_BIN_EXE_lxagents-gitlab");
const ACCESS: &str = "f00dfacecafe0123456789abcdef999999999999999999999999999999999999";

/* ------------------------------------------------------------------ the client */

struct Answer {
    status: u16,
    headers: BTreeMap<String, String>,
    text: String,
    json: Value,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(&name.to_lowercase()).map(String::as_str)
    }
}

/// Decode a chunked body.
fn unchunk(mut raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let Some(end) = raw.windows(2).position(|w| w == b"\r\n") else {
            return out;
        };
        let size = usize::from_str_radix(
            std::str::from_utf8(&raw[..end])
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .trim(),
            16,
        )
        .unwrap_or(0);
        if size == 0 {
            return out;
        }
        raw = &raw[end + 2..];
        out.extend_from_slice(&raw[..size.min(raw.len())]);
        raw = &raw[(size + 2).min(raw.len())..];
    }
}

/// One request on its own connection. `Host` is sent as `127.0.0.1:<port>` unless `headers` names one;
/// a `Host` of "" sends none at all.
async fn send(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Answer {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect");
    let mut head = format!("{method} {path} HTTP/1.1\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        head.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("host") && v.is_empty() {
            continue;
        }
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() || method == "POST" {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Connection: close\r\n\r\n");
    stream.write_all(head.as_bytes()).await.unwrap();
    // A server may answer and close before it reads all of a body it refuses.
    let _ = stream.write_all(body).await;
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw).await;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response head");
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    let mut body = raw[split + 4..].to_vec();
    if headers
        .get("transfer-encoding")
        .is_some_and(|v| v.contains("chunked"))
    {
        body = unchunk(&body);
    }
    let text = String::from_utf8_lossy(&body).to_string();
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Answer {
        status,
        headers,
        text,
        json,
    }
}

const JSON_RPC: [(&str, &str); 2] = [
    ("Content-Type", "application/json"),
    ("Accept", "application/json, text/event-stream"),
];

fn with_json(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    let mut headers = JSON_RPC.to_vec();
    headers.extend_from_slice(extra);
    headers
}

/// A JSON-RPC call to `/mcp`, carrying a bearer credential or none.
async fn rpc(port: u16, bearer: Option<&str>, method: &str, params: Value) -> Answer {
    let authorization = bearer.map(|b| format!("Bearer {b}"));
    let mut headers: Vec<(&str, &str)> = JSON_RPC.to_vec();
    if let Some(value) = authorization.as_deref() {
        headers.push(("Authorization", value));
    }
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
    send(port, "POST", "/mcp", &headers, body.as_bytes()).await
}

fn read_repo(name: &str) -> Value {
    json!({ "name": "repo_read", "arguments": { "repo": format!("o/{name}") } })
}

fn listed(answer: &Answer) -> Vec<String> {
    let mut names: Vec<String> = answer.json["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

/* ------------------------------------------------------------------ the servers */

fn options(t: &Harness, auth_token: &str) -> HttpOptions {
    let mut options = HttpOptions::new(t.ctx.clone());
    options.port = 0;
    options.host = "127.0.0.1".into();
    options.auth_token = Some(auth_token.into());
    options.logger = Arc::new(|_: &str| {});
    options
}

/// A running server on a free loopback port, guarded by ACCESS.
async fn serve(t: &Harness) -> RunningServer {
    start_http_server(options(t, ACCESS))
        .await
        .expect("listens")
}

/// A local install on a free loopback port: no access token at all.
async fn serve_local(t: &Harness) -> RunningServer {
    start_http_server(options(t, "")).await.expect("listens")
}

/* ------------------------------------------------------------------ the routes */

#[tokio::test]
async fn get_healthz_is_open_and_says_nothing_but_who_is_there() {
    let t = Harness::new();
    let server = serve(&t).await;
    let answer = send(server.port, "GET", "/healthz", &[], b"").await;
    assert_eq!(answer.status, 200);
    assert_eq!(
        answer.json,
        json!({ "status": "ok", "server": SERVER_NAME, "version": VERSION })
    );
    server.close().await;
}

#[tokio::test]
async fn the_exemption_is_exactly_get_healthz() {
    let t = Harness::new();
    let server = serve(&t).await;
    for (method, path) in [
        ("HEAD", "/healthz"),
        ("GET", "/healthz/"),
        ("POST", "/healthz"),
        ("GET", "/healthz2"),
    ] {
        assert_eq!(
            send(server.port, method, path, &[], b"").await.status,
            401,
            "{method} {path}"
        );
    }
    server.close().await;
}

#[tokio::test]
async fn a_request_without_the_access_token_is_a_401_in_the_json_rpc_envelope_before_anything_is_read()
 {
    let t = Harness::new();
    let server = serve(&t).await;
    let missing = send(server.port, "POST", "/mcp", &JSON_RPC, b"this is not json").await;
    assert_eq!(
        missing.status, 401,
        "the token is checked before the body is parsed"
    );
    assert_eq!(
        missing.header("www-authenticate"),
        Some("Bearer realm=\"mcp\"")
    );
    assert_eq!(
        missing.json,
        json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32001, "message": "Unauthorized: send the server's access token as Authorization: Bearer <token>" } })
    );
    for authorization in ["Basic dXNlcjpwYXNz", "Bearer", "Bearer a b", "Token abc"] {
        let wrong = send(
            server.port,
            "POST",
            "/mcp",
            &with_json(&[("Authorization", authorization)]),
            b"{}",
        )
        .await;
        assert_eq!(wrong.status, 401, "{authorization}");
        assert_eq!(
            wrong.header("www-authenticate"),
            Some("Bearer realm=\"mcp\""),
            "{authorization}"
        );
    }
    server.close().await;
}

#[tokio::test]
async fn a_wrong_token_is_a_401_with_invalid_token_whatever_it_resembles_and_the_answer_never_echoes_either_token()
 {
    let t = Harness::new();
    let server = serve(&t).await;
    let upper: String = ACCESS
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { 'Z' })
        .collect();
    let wrong_tokens = [
        "not-the-token".to_string(),
        ACCESS[..ACCESS.len() - 1].to_string(),
        format!("{ACCESS}x"),
        upper,
        "a".repeat(2000),
        "tokén".to_string(),
        TOKEN.to_string(),
    ];
    for token in &wrong_tokens {
        let authorization = format!("Bearer {token}");
        let answer = send(
            server.port,
            "POST",
            "/mcp",
            &[JSON_RPC[0], JSON_RPC[1], ("Authorization", &authorization)],
            b"{}",
        )
        .await;
        let label: String = token.chars().take(20).collect();
        assert_eq!(answer.status, 401, "{label}");
        assert_eq!(
            answer.header("www-authenticate"),
            Some("Bearer realm=\"mcp\", error=\"invalid_token\"")
        );
        assert_eq!(
            answer.json,
            json!({ "jsonrpc": "2.0", "error": { "code": -32001, "message": "Unauthorized: the token is not valid" }, "id": null })
        );
        assert!(!answer.text.contains(ACCESS), "the access token");
        assert!(
            !answer.text.contains(token.as_str()),
            "the token that was sent"
        );
    }
    server.close().await;
}

#[tokio::test]
async fn a_caller_without_the_token_cannot_learn_which_routes_exist() {
    let t = Harness::new();
    let server = serve(&t).await;
    assert_eq!(
        send(server.port, "GET", "/nope", &[], b"").await.status,
        401
    );
    assert_eq!(send(server.port, "GET", "/mcp", &[], b"").await.status, 401);
    assert_eq!(
        send(server.port, "DELETE", "/mcp", &[], b"").await.status,
        401
    );
    server.close().await;
}

#[tokio::test]
async fn with_the_token_an_unknown_path_is_a_404_and_a_method_other_than_post_is_a_405() {
    let t = Harness::new();
    let server = serve(&t).await;
    let bearer = format!("Bearer {ACCESS}");
    let headers = [JSON_RPC[0], JSON_RPC[1], ("Authorization", bearer.as_str())];
    let missing = send(server.port, "GET", "/nope", &headers, b"").await;
    assert_eq!(missing.status, 404);
    assert_eq!(missing.json["error"]["message"], json!("Not found"));
    for method in ["GET", "DELETE", "PUT"] {
        let refused = send(server.port, method, "/mcp", &headers, b"").await;
        assert_eq!(refused.status, 405, "{method}");
        assert_eq!(refused.header("allow"), Some("POST"));
    }
    server.close().await;
}

#[tokio::test]
async fn a_body_that_is_not_json_is_a_400_and_one_over_the_limit_is_a_413() {
    let t = Harness::new();
    let server = serve(&t).await;
    let bearer = format!("Bearer {ACCESS}");
    let headers = [JSON_RPC[0], JSON_RPC[1], ("Authorization", bearer.as_str())];
    let malformed = send(server.port, "POST", "/mcp", &headers, b"{not json").await;
    assert_eq!(malformed.status, 400);
    assert_eq!(malformed.json["error"]["code"], json!(-32700));
    let oversized = send(
        server.port,
        "POST",
        "/mcp",
        &headers,
        &vec![b'a'; MAX_BODY_BYTES + 1],
    )
    .await;
    assert_eq!(oversized.status, 413);
    server.close().await;
}

/* ------------------------------------------------------------------ the tools, with the access token */

#[tokio::test]
async fn tools_list_over_http_lists_exactly_the_surface() {
    let t = Harness::new();
    let server = serve(&t).await;
    let answer = rpc(server.port, Some(ACCESS), "tools/list", json!({})).await;
    assert_eq!(answer.status, 200);
    assert_eq!(listed(&answer), tool_names());
    server.close().await;
}

#[tokio::test]
async fn gitlab_is_called_with_the_key_and_the_clients_access_token_never_reaches_it() {
    let t = Harness::with_token();
    t.mock(vec![reply(404, json!({ "message": "Not Found" }))]);
    let server = serve(&t).await;
    let answer = rpc(server.port, Some(ACCESS), "tools/call", read_repo("r")).await;
    assert_eq!(answer.status, 200);
    assert_eq!(answer.json["result"]["isError"], json!(true));
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, format!("{API}/projects/o%2Fr?license=true"));
    assert_eq!(
        calls[0].header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str()),
        "the key, not the access token"
    );
    assert!(
        !format!("{calls:?}").contains(ACCESS),
        "the access token is in no part of the request to GitLab"
    );
    server.close().await;
}

#[tokio::test]
async fn with_the_access_token_but_no_gitlab_key_a_tool_is_refused_naming_the_key_and_gitlab_is_not_called()
 {
    let t = Harness::new();
    let server = serve(&t).await;
    let answer = rpc(server.port, Some(ACCESS), "tools/call", read_repo("r")).await;
    assert_eq!(answer.status, 200);
    assert_eq!(answer.json["result"]["isError"], json!(true));
    assert_match(
        answer.json["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
        "GITLAB_API_KEY is not set",
    );
    assert_eq!(t.call_count(), 0);
    server.close().await;
}

#[tokio::test]
async fn a_request_without_the_access_token_never_reaches_gitlab_even_when_the_key_is_set() {
    let t = Harness::with_token();
    let server = serve(&t).await;
    assert_eq!(
        rpc(server.port, None, "tools/call", read_repo("r"))
            .await
            .status,
        401
    );
    assert_eq!(
        rpc(server.port, Some(TOKEN), "tools/call", read_repo("r"))
            .await
            .status,
        401,
        "the GitLab key is not an access token"
    );
    assert_eq!(t.call_count(), 0);
    server.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requests_that_overlap_all_reach_gitlab_with_the_key_and_none_with_an_access_token() {
    let t = Harness::with_token();
    t.mock(
        (0..8)
            .map(|_| reply(404, json!({ "message": "Not Found" })))
            .collect(),
    );
    let server = serve(&t).await;
    let port = server.port;
    let names: Vec<String> = (0..8).map(|i| format!("r{i}")).collect();
    let answers = futures::future::join_all(
        names
            .iter()
            .map(|name| rpc(port, Some(ACCESS), "tools/call", read_repo(name))),
    )
    .await;
    assert!(answers.iter().all(|answer| answer.status == 200));
    let seen: BTreeMap<String, String> = t
        .calls()
        .iter()
        .map(|call| {
            (
                call.path(),
                call.header("Authorization").unwrap_or("").to_string(),
            )
        })
        .collect();
    for name in &names {
        assert_eq!(
            seen.get(&format!("/api/v4/projects/o%2F{name}"))
                .map(String::as_str),
            Some(format!("Bearer {TOKEN}").as_str()),
            "{name}"
        );
    }
    server.close().await;
}

#[tokio::test]
async fn gitlab_rejecting_the_key_names_it_and_neither_secret_reaches_the_caller_or_the_log() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        401,
        json!({ "message": format!("Bad credentials for {TOKEN}") }),
    )]);
    let logged = Arc::new(Mutex::new(String::new()));
    let sink = logged.clone();
    let mut options = options(&t, ACCESS);
    let logger: Logger = Arc::new(move |line: &str| sink.lock().unwrap().push_str(line));
    options.logger = logger;
    let server = start_http_server(options).await.unwrap();
    let answer = rpc(server.port, Some(ACCESS), "tools/call", read_repo("r")).await;
    assert_eq!(answer.status, 200);
    assert_match(
        answer.json["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
        r"GitLab rejected the token \(401\)\. Check that GITLAB_API_KEY holds a valid token",
    );
    for secret in [TOKEN, ACCESS] {
        assert!(!answer.text.contains(secret), "the response");
        assert!(!logged.lock().unwrap().contains(secret), "the log");
    }
    server.close().await;
}

/* ------------------------------------------------------------------ starting */

#[tokio::test]
async fn start_http_server_will_not_listen_without_an_access_token_of_32_characters_and_names_the_variable()
 {
    let t = Harness::new();
    let mut open = options(&t, "");
    open.host = "0.0.0.0".into();
    assert_eq!(
        start_http_server(open).await.err().unwrap(),
        "MCP_AUTH_TOKEN is not set"
    );
    let mut short = options(&t, "short");
    short.host = "0.0.0.0".into();
    assert_match(
        &start_http_server(short).await.err().unwrap(),
        "MCP_AUTH_TOKEN is 5 characters, and it must be at least 32",
    );
}

fn settings(pairs: &[(&str, &str)]) -> Result<HttpSettings, String> {
    let map: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    http_settings(&move |name: &str| map.get(name).cloned())
}

fn at(port: u16, host: &str) -> (u16, String) {
    (port, host.to_string())
}

fn read(pairs: &[(&str, &str)]) -> (u16, String) {
    let found = settings(pairs).unwrap();
    (found.port, found.host)
}

#[test]
fn http_settings_with_an_access_token_defaults_to_port_3000_on_every_interface() {
    assert_eq!(read(&[(AUTH_ENV, ACCESS)]), at(3000, "0.0.0.0"));
    assert_eq!(
        read(&[(AUTH_ENV, ACCESS), ("PORT", ""), ("HOST", "")]),
        at(3000, "0.0.0.0")
    );
    assert_eq!(
        read(&[(AUTH_ENV, ACCESS), ("PORT", "   "), ("HOST", "   ")]),
        at(3000, "0.0.0.0")
    );
    assert_eq!(DEFAULT_PORT, 3000);
    assert_eq!(DEFAULT_HOST, "0.0.0.0");
}

#[test]
fn http_settings_without_an_access_token_defaults_to_this_machine_only_a_local_install() {
    assert_eq!(read(&[]), at(3000, "127.0.0.1"));
    assert_eq!(read(&[("PORT", ""), ("HOST", "")]), at(3000, "127.0.0.1"));
    assert_eq!(
        read(&[(AUTH_ENV, "   ")]),
        at(3000, "127.0.0.1"),
        "a blank token is no token"
    );
    assert_eq!(LOCAL_HOST, "127.0.0.1");
    assert_eq!(
        read(&[("HOST", "0.0.0.0")]),
        at(3000, "0.0.0.0"),
        "HOST still wins; the start refuses it"
    );
}

#[test]
fn http_settings_lets_port_and_host_override_accepts_0_and_refuses_a_port_that_is_not_a_port() {
    assert_eq!(
        read(&[("PORT", " 8080 "), ("HOST", " 127.0.0.1 ")]),
        at(8080, "127.0.0.1")
    );
    assert_eq!(
        read(&[("PORT", "0"), (AUTH_ENV, ACCESS)]),
        at(0, "0.0.0.0"),
        "0 asks the system for a free port"
    );
    for bad in ["abc", "70000", "-1", "80.5", "1e9"] {
        let error = settings(&[("PORT", bad)])
            .err()
            .unwrap_or_else(|| panic!("{bad} was taken"));
        assert!(
            error.contains(&format!(
                "PORT must be a whole number from 0 to 65535, got '{bad}'"
            )),
            "{error}"
        );
    }
}

/* ------------------------------------------------------------------ the process */

/// A real process with exactly the given environment on top of ours, minus both secrets and the
/// transport settings. stderr is collected on a thread as it arrives.
struct Process {
    child: Child,
    stderr: Arc<Mutex<String>>,
}

impl Process {
    fn start(env: &[(&str, &str)]) -> Process {
        let mut command = Command::new(BIN);
        for name in [TOKEN_ENV, AUTH_ENV, "MCP_TRANSPORT", "PORT", "HOST"] {
            command.env_remove(name);
        }
        command
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let stderr = Arc::new(Mutex::new(String::new()));
        let sink = stderr.clone();
        let mut pipe = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                sink.lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&buffer[..n]));
            }
        });
        Process { child, stderr }
    }

    fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }

    /// Wait until stderr matches `pattern`, and return what matched first in it.
    fn wait_for(&self, pattern: &str) -> String {
        let re = regex::Regex::new(pattern).unwrap();
        let start = Instant::now();
        loop {
            let text = self.stderr();
            if let Some(found) = re.captures(&text) {
                return found.get(1).or(found.get(0)).unwrap().as_str().to_string();
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "no startup line: {text}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The exit code of a process that should end by itself, or `None` if it is still up after ten seconds.
    fn exit_code(&mut self) -> Option<i32> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if let Some(status) = self.child.try_wait().unwrap() {
                // Let the reader thread drain the pipe.
                std::thread::sleep(Duration::from_millis(50));
                return status.code();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    fn term(&mut self) -> Option<i32> {
        Command::new("kill")
            .arg("-TERM")
            .arg(self.child.id().to_string())
            .status()
            .unwrap();
        self.exit_code()
    }

    fn stdout(&mut self) -> String {
        let mut text = String::new();
        if let Some(mut out) = self.child.stdout.take() {
            out.read_to_string(&mut text).ok();
        }
        text
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

/// A port nothing is listening on right now.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn port_of(base: &str) -> u16 {
    base.rsplit(':').next().unwrap().parse().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_transport_http_serves_http_from_a_real_process_behind_the_access_token_and_stops_on_sigterm()
 {
    let key = "glpat-process_key_value_9876";
    let mut run = Process::start(&[
        ("MCP_TRANSPORT", "http"),
        ("PORT", "0"),
        ("HOST", "127.0.0.1"),
        (AUTH_ENV, ACCESS),
        (TOKEN_ENV, key),
    ]);
    let base = run.wait_for(r"serving over http on (http://127\.0\.0\.1:\d+)/mcp");
    let port = port_of(&base);
    assert_eq!(send(port, "GET", "/healthz", &[], b"").await.status, 200);
    assert_eq!(rpc(port, None, "tools/list", json!({})).await.status, 401);
    assert_eq!(
        rpc(port, Some(key), "tools/list", json!({})).await.status,
        401,
        "the GitLab key is not a way in"
    );
    assert_eq!(
        rpc(port, Some(ACCESS), "tools/list", json!({}))
            .await
            .status,
        200
    );
    assert_eq!(run.term(), Some(0));
    assert_eq!(run.stdout(), "", "nothing on stdout");
    let stderr = run.stderr();
    assert!(
        !stderr.contains(ACCESS) && !stderr.contains(key),
        "neither secret is ever logged"
    );
    assert_no_match(&stderr, "GITLAB_API_KEY is not set");
}

#[tokio::test(flavor = "multi_thread")]
async fn with_no_host_set_a_real_process_listens_on_every_interface_and_says_so_with_host_set_only_there()
 {
    for (host, said) in [
        (None, r"serving over http on :\d+/mcp \(all interfaces\)"),
        (
            Some("127.0.0.1"),
            r"serving over http on http://127\.0\.0\.1:\d+/mcp",
        ),
    ] {
        let port = free_port().to_string();
        let mut env = vec![
            ("MCP_TRANSPORT", "http"),
            ("PORT", port.as_str()),
            (AUTH_ENV, ACCESS),
        ];
        if let Some(host) = host {
            env.push(("HOST", host));
        }
        let mut run = Process::start(&env);
        run.wait_for("serving over http on");
        assert_match(&run.stderr(), said);
        assert_eq!(
            send(port.parse().unwrap(), "GET", "/healthz", &[], b"")
                .await
                .status,
            200
        );
        run.term();
    }
}

#[test]
fn http_without_a_gitlab_key_starts_and_says_so_because_a_tool_will_refuse() {
    let mut run = Process::start(&[("MCP_TRANSPORT", "http"), ("PORT", "0"), (AUTH_ENV, ACCESS)]);
    run.wait_for("serving over http on");
    assert_match(
        &run.stderr(),
        "GITLAB_API_KEY is not set in this process's environment: every tool will refuse until it is",
    );
    run.term();
}

#[test]
fn http_other_machines_can_reach_will_not_start_without_a_usable_access_token_and_a_set_token_is_always_checked()
 {
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (vec![("HOST", "0.0.0.0")], "MCP_AUTH_TOKEN is not set"),
        (
            vec![("HOST", "0.0.0.0"), (AUTH_ENV, "   ")],
            "MCP_AUTH_TOKEN is not set",
        ),
        (vec![("HOST", "192.0.2.1")], "MCP_AUTH_TOKEN is not set"),
        (
            vec![(AUTH_ENV, "too-short")],
            "MCP_AUTH_TOKEN is 9 characters, and it must be at least 32",
        ),
        (
            vec![("HOST", "127.0.0.1"), (AUTH_ENV, "too-short")],
            "MCP_AUTH_TOKEN is 9 characters, and it must be at least 32",
        ),
        (
            vec![("HOST", "0.0.0.0"), (TOKEN_ENV, "glpat-only_the_key_1234")],
            "MCP_AUTH_TOKEN is not set",
        ),
    ];
    for (extra, reason) in cases {
        let mut env = vec![("MCP_TRANSPORT", "http"), ("PORT", "0")];
        env.extend(extra.iter().copied());
        let mut run = Process::start(&env);
        assert_eq!(run.exit_code(), Some(1), "{extra:?}");
        let stderr = run.stderr();
        assert_match(&stderr, reason);
        assert_match(&stderr, "will not start the HTTP transport");
        assert_match(&stderr, "openssl rand -hex 32");
        assert_match(
            &stderr,
            "A local install needs no access token: leave HOST unset",
        );
        assert_eq!(run.stdout(), "");
        assert_no_match(&stderr, "serving over http");
        assert!(!stderr.contains("too-short"), "the value is never logged");
    }
}

#[test]
fn an_unknown_mcp_transport_or_a_port_that_is_not_a_port_exits_2_with_the_reason_on_stderr() {
    let cases: Vec<(Vec<(&str, &str)>, &str)> = vec![
        (
            vec![("MCP_TRANSPORT", "carrier-pigeon")],
            "MCP_TRANSPORT must be stdio or http, got 'carrier-pigeon'",
        ),
        (
            vec![
                ("MCP_TRANSPORT", "http"),
                ("PORT", "http"),
                (AUTH_ENV, ACCESS),
            ],
            "PORT must be a whole number from 0 to 65535, got 'http'",
        ),
        (
            vec![
                ("MCP_TRANSPORT", "http"),
                ("PORT", "70000"),
                (AUTH_ENV, ACCESS),
            ],
            "PORT must be a whole number from 0 to 65535, got '70000'",
        ),
    ];
    for (env, reason) in cases {
        let mut run = Process::start(&env);
        assert_eq!(run.exit_code(), Some(2), "{env:?}");
        assert_match(&run.stderr(), reason);
        assert_eq!(run.stdout(), "");
    }
}

/* ------------------------------------------------------------------ a local install: no access token */

#[tokio::test]
async fn start_http_server_takes_no_access_token_only_on_loopback() {
    let t = Harness::new();
    for host in ["0.0.0.0", "192.0.2.1"] {
        let mut open = options(&t, "");
        open.host = host.into();
        assert_eq!(
            start_http_server(open).await.err().as_deref(),
            Some("MCP_AUTH_TOKEN is not set"),
            "{host}"
        );
    }
    let server = serve_local(&t).await;
    assert_eq!(
        send(server.port, "GET", "/healthz", &[], b"").await.status,
        200
    );
    server.close().await;
}

#[tokio::test]
async fn a_local_install_serves_a_request_addressed_to_loopback_without_any_token() {
    let t = Harness::new();
    let server = serve_local(&t).await;
    let port = server.port;
    let body =
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }).to_string();
    for host in [
        None,
        Some(format!("localhost:{port}")),
        Some("localhost".to_string()),
        Some(format!("127.0.0.1:{port}")),
        Some(format!("[::1]:{port}")),
    ] {
        let mut headers = JSON_RPC.to_vec();
        if let Some(host) = host.as_deref() {
            headers.push(("Host", host));
        }
        let answer = send(port, "POST", "/mcp", &headers, body.as_bytes()).await;
        assert_eq!(answer.status, 200, "{host:?}");
        assert_eq!(listed(&answer), tool_names());
    }
    let from_local_page = send(
        port,
        "POST",
        "/mcp",
        &with_json(&[("Origin", "http://localhost:5173")]),
        body.as_bytes(),
    )
    .await;
    assert_eq!(
        from_local_page.status, 200,
        "a page served from this machine"
    );
    server.close().await;
}

#[tokio::test]
async fn a_local_install_refuses_a_request_a_web_page_could_have_rebound_to_it_before_anything_is_read()
 {
    let t = Harness::new();
    let server = serve_local(&t).await;
    let port = server.port;
    let probes: Vec<(&str, String)> = vec![
        ("Host", "evil.example".into()),
        ("Host", format!("evil.example:{port}")),
        ("Host", format!("localhost.evil.example:{port}")),
        ("Host", format!("localhost@evil.example:{port}")),
        ("Host", format!("127.0.0.1.nip.io:{port}")),
        ("Origin", "http://evil.example".into()),
        ("Origin", format!("http://evil.example:{port}")),
        ("Origin", "null".into()),
        ("Origin", "file://".into()),
    ];
    for (name, value) in &probes {
        let mut headers = JSON_RPC.to_vec();
        headers.push((name, value.as_str()));
        let answer = send(port, "POST", "/mcp", &headers, b"this is not json").await;
        assert_eq!(answer.status, 403, "{name}: {value}");
        assert_eq!(
            answer.json["error"]["code"],
            json!(-32001),
            "{name}: {value}"
        );
        assert_match(
            answer.json["error"]["message"].as_str().unwrap(),
            "^Forbidden: this server has no access token",
        );
    }
    // The one open route stays open, as it is with a token.
    assert_eq!(
        send(port, "GET", "/healthz", &[("Host", "evil.example")], b"")
            .await
            .status,
        200
    );
    server.close().await;
}

#[tokio::test]
async fn a_local_install_still_routes_like_a_server_install_once_the_request_is_addressed_to_loopback()
 {
    let t = Harness::new();
    let server = serve_local(&t).await;
    assert_eq!(send(server.port, "GET", "/mcp", &[], b"").await.status, 405);
    assert_eq!(
        send(server.port, "POST", "/other", &JSON_RPC, b"{}")
            .await
            .status,
        404
    );
    assert_eq!(
        send(server.port, "POST", "/mcp", &JSON_RPC, b"this is not json")
            .await
            .status,
        400
    );
    server.close().await;
}

#[test]
fn local_request_problem_reads_host_whole_and_origin_only_as_a_web_origin() {
    assert_eq!(local_request_problem(Some("127.0.0.1:3000"), None), None);
    assert_eq!(
        local_request_problem(Some("[::1]:3000"), Some("https://localhost")),
        None
    );
    assert_match(
        local_request_problem(None, None).unwrap(),
        "addressed to localhost",
    );
    assert_match(
        local_request_problem(Some("127.0.0.1:3000/evil"), None).unwrap(),
        "addressed to localhost",
    );
    assert_match(
        local_request_problem(Some("127.0.0.1:3000"), Some("ftp://localhost")).unwrap(),
        "another origin",
    );
    assert_match(
        local_request_problem(Some("127.0.0.1:3000"), Some("not a url")).unwrap(),
        "another origin",
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn with_neither_token_nor_host_a_real_process_is_a_local_install_loopback_no_token_and_it_says_so()
 {
    let port = free_port();
    let port_text = port.to_string();
    let mut run = Process::start(&[
        ("MCP_TRANSPORT", "http"),
        ("PORT", port_text.as_str()),
        (TOKEN_ENV, "glpat-process_key_value_9876"),
    ]);
    run.wait_for("local install");
    let stderr = run.stderr();
    assert_match(
        &stderr,
        &format!(r"serving over http on http://127\.0\.0\.1:{port}/mcp"),
    );
    assert_match(
        &stderr,
        "local install: MCP_AUTH_TOKEN is not set, so only this machine can reach the server",
    );
    assert_no_match(&stderr, "all interfaces");
    assert_eq!(rpc(port, None, "tools/list", json!({})).await.status, 200);
    assert_eq!(run.term(), Some(0));
    assert_eq!(run.stdout(), "");
}
