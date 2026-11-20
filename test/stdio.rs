//! The server as a client actually runs it: a real process, spoken to over a real pipe. The things
//! that go wrong here - a stray write to stdout, a process that outlives its client - do not
//! reproduce on an in-memory transport.
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use lxagents_gitlab::config::config::TOKEN_ENV;
use lxagents_gitlab::config::version::{SERVER_NAME, VERSION};
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::TokioChildProcess;
use serde_json::json;

use crate::helpers::assert_match;
use crate::surface::tool_names;

const BIN: &str = env!("CARGO_BIN_EXE_lxagents-gitlab");

/// The binary with no token and no transport choice in its environment.
fn command() -> Command {
    let mut command = Command::new(BIN);
    command
        .env_remove(TOKEN_ENV)
        .env_remove("MCP_TRANSPORT")
        .env_remove("PORT");
    command
}

/// Wait for `child` to exit, killing it after five seconds.
fn wait(child: &mut std::process::Child) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.kill().ok();
    None
}

#[tokio::test]
async fn over_stdio_the_server_lists_exactly_its_surface_and_answers_a_call() {
    let mut command = tokio::process::Command::new(BIN);
    command.env_remove(TOKEN_ENV).env_remove("MCP_TRANSPORT");
    let (transport, _stderr) = TokioChildProcess::builder(command)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let client = ().serve(transport).await.unwrap();
    let mut names: Vec<String> = client
        .list_all_tools()
        .await
        .unwrap()
        .iter()
        .map(|tool| tool.name.to_string())
        .collect();
    names.sort();
    assert_eq!(names, tool_names());
    // The process inherits no token, so the call is refused - which is the answer being checked: a
    // refusal that names the variable, not a crash and not a hang.
    let args = json!({ "name": "demo" }).as_object().cloned().unwrap();
    let result = client
        .call_tool(CallToolRequestParams::new("repo_create").with_arguments(args))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    let value = serde_json::to_value(&result).unwrap();
    assert_match(
        value["content"][0]["text"].as_str().unwrap(),
        "GITLAB_API_KEY is not set",
    );
    client.cancel().await.ok();
}

#[test]
fn a_bad_command_line_exits_2_with_the_reason_on_stderr_and_nothing_on_stdout() {
    for args in [vec!["--bogus"], vec!["stray"], vec!["--help=1"]] {
        let run = command().args(&args).output().unwrap();
        assert_eq!(run.status.code(), Some(2), "{args:?}");
        assert!(run.stdout.is_empty(), "{args:?} wrote to stdout");
        assert_match(
            &String::from_utf8_lossy(&run.stderr),
            "Usage: lxagents-gitlab",
        );
    }
}

#[test]
fn help_prints_the_usage_to_stderr_leaves_stdout_empty_and_exits_0() {
    let run = command().arg("--help").output().unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.is_empty());
    assert_match(&String::from_utf8_lossy(&run.stderr), "GITLAB_API_KEY");
}

#[test]
fn version_prints_the_name_and_version_to_stderr_leaves_stdout_empty_and_exits_0() {
    let run = command().arg("--version").output().unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert!(run.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&run.stderr).trim(),
        format!("{SERVER_NAME} {VERSION}")
    );
}

/// A signal closes the server and exits 0, which a container's first process needs.
fn closes_on(signal: &str) {
    let mut child = command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The handlers are installed before this line is written, so it is safe to signal after it.
    let mut stderr = child.stderr.take().unwrap();
    let mut seen = String::new();
    let mut byte = [0u8; 1];
    while !seen.contains("serving over stdio") {
        assert_eq!(
            stderr.read(&mut byte).unwrap(),
            1,
            "the server stopped before it served: {seen}"
        );
        seen.push(byte[0] as char);
    }
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    let exited = wait(&mut child).expect("still running");
    assert_eq!(exited.code(), Some(0), "{signal}");
}

#[test]
fn sigint_closes_the_server_and_exits_0() {
    closes_on("INT");
}

#[test]
fn sigterm_closes_the_server_and_exits_0() {
    closes_on("TERM");
}

#[test]
fn the_process_exits_by_itself_when_its_client_goes_away() {
    let mut child = command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let exited = wait(&mut child).expect("still running");
    assert_eq!(exited.code(), Some(0));
}
