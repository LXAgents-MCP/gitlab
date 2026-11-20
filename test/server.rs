//! The MCP server as a client sees it, over an in-memory pipe.
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use lxagents_gitlab::config::version::{SERVER_NAME, VERSION};
use lxagents_gitlab::controllers::ToolSpec;
use lxagents_gitlab::models::args::Args;
use lxagents_gitlab::models::outcome::Outcome;
use lxagents_gitlab::routes::server::{GitlabServer, Logger, install_panic_hook, tool_result};
use lxagents_gitlab::services::context::Ctx;
use lxagents_gitlab::utils::errors::Refused;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use serde_json::{Map, Value, json};

use crate::helpers::*;
use crate::surface::{kind, kind_hints, tool_names};

type Client = RunningService<RoleClient, ()>;

/// A client connected to `server` over an in-memory pipe.
async fn connect_to(server: GitlabServer) -> Client {
    let (client_io, server_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        if let Ok(running) = server.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    ().serve(client_io).await.expect("client connects")
}

async fn connect(t: &Harness) -> Client {
    connect_to(GitlabServer::new(t.ctx.clone())).await
}

async fn call(client: &Client, name: &'static str, args: Value) -> CallToolResult {
    let params = CallToolRequestParams::new(name)
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    client.call_tool(params).await.expect("call answers")
}

/// A tool result's one text block.
fn text(result: &CallToolResult) -> String {
    let value = serde_json::to_value(result).unwrap();
    let content = value["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "expected exactly one content block");
    assert_eq!(content[0]["type"], json!("text"));
    content[0]["text"].as_str().unwrap().to_string()
}

fn is_error(result: &CallToolResult) -> bool {
    result.is_error == Some(true)
}

/// A server whose repo_create crashes with `handle`, and the lines it logs.
fn crashing(
    t: &Harness,
    handle: for<'a> fn(&'a Ctx, &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>>,
) -> (GitlabServer, Arc<Mutex<Vec<String>>>) {
    install_panic_hook();
    let mut server = GitlabServer::new(t.ctx.clone());
    let tools: Vec<ToolSpec> = server
        .tools
        .iter()
        .cloned()
        .map(|mut spec| {
            if spec.name() == "repo_create" {
                spec.handle = handle;
            }
            spec
        })
        .collect();
    server.tools = Arc::new(tools);
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let logger: Logger = Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_string()));
    server.logger = logger;
    (server, lines)
}

#[tokio::test]
async fn the_server_identifies_itself_offers_tools_only_and_tells_a_client_how_to_use_it() {
    let t = Harness::new();
    let client = connect(&t).await;
    let info = serde_json::to_value(client.peer_info().expect("initialized").as_ref()).unwrap();
    assert_eq!(info["serverInfo"]["name"], json!(SERVER_NAME));
    assert_eq!(info["serverInfo"]["version"], json!(VERSION));
    let capabilities: Vec<&String> = info["capabilities"].as_object().unwrap().keys().collect();
    assert_eq!(capabilities, ["tools"]);
    let instructions = info["instructions"].as_str().unwrap_or("");
    assert_match(instructions, "GITLAB_API_KEY");
    assert_match(instructions, "'group/project'");
    client.cancel().await.ok();
}

#[tokio::test]
async fn tools_list_advertises_exactly_the_surface_with_schemas_and_hints() {
    let t = Harness::new();
    let client = connect(&t).await;
    let tools = client.list_all_tools().await.unwrap();
    let mut names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
    names.sort();
    assert_eq!(names, tool_names());
    for tool in &tools {
        let value = serde_json::to_value(tool).unwrap();
        assert_eq!(
            value["outputSchema"]["type"],
            json!("object"),
            "{} has no output schema",
            tool.name
        );
        assert!(value["description"].as_str().unwrap_or("").chars().count() > 20);
        assert_eq!(
            value["annotations"],
            kind_hints(kind(&tool.name)),
            "{}",
            tool.name
        );
    }
    let create = tools
        .iter()
        .find(|tool| tool.name == "repo_create")
        .unwrap();
    assert_eq!(create.input_schema.get("required"), Some(&json!(["name"])));
    client.cancel().await.ok();
}

#[tokio::test]
async fn repo_create_over_the_protocol_puts_the_repository_in_the_text_and_in_the_structured_content()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(
        201,
        json!({
            "id": 7, "name": "demo", "path": "demo", "path_with_namespace": "acme/demo", "web_url": "https://gitlab.com/acme/demo",
            "http_url_to_repo": "https://gitlab.com/acme/demo.git", "visibility": "private", "default_branch": "main",
        }),
    )]);
    let client = connect(&t).await;
    let result = call(&client, "repo_create", json!({ "name": "demo" })).await;
    assert!(!is_error(&result));
    assert_eq!(
        text(&result),
        "**Created private repository acme/demo on GitLab.**\n\n\
         - name: demo\n\
         - full_name: acme/demo\n\
         - html_url: https://gitlab.com/acme/demo\n\
         - clone_url: https://gitlab.com/acme/demo.git\n\
         - visibility: private\n\
         - default_branch: main"
    );
    let structured = result.structured_content.clone().unwrap();
    assert_eq!(
        structured["html_url"],
        json!("https://gitlab.com/acme/demo")
    );
    assert!(structured.get("summary").is_none());
    client.cancel().await.ok();
}

#[tokio::test]
async fn a_refusal_is_an_error_result_with_the_reason_and_no_structured_content() {
    let t = Harness::new();
    let client = connect(&t).await;
    let result = call(&client, "repo_create", json!({ "name": "demo" })).await;
    assert!(is_error(&result));
    assert!(result.structured_content.is_none());
    assert_match(
        &text(&result),
        r"^\*\*Refused\*\*\n\nGITLAB_API_KEY is not set\.",
    );
    client.cancel().await.ok();
}

#[tokio::test]
async fn a_call_with_a_missing_argument_or_to_no_such_tool_is_an_error_result_and_not_a_crash() {
    let t = Harness::new();
    let client = connect(&t).await;
    let missing = call(&client, "repo_create", json!({})).await;
    assert!(is_error(&missing));
    assert_match(&text(&missing), "name");
    let unknown = call(&client, "no_such_tool", json!({})).await;
    assert!(is_error(&unknown));
    assert_match(&text(&unknown), "(?i)not found");
    assert_eq!(
        client.list_all_tools().await.unwrap().len(),
        tool_names().len(),
        "the server is still serving"
    );
    client.cancel().await.ok();
}

fn boom<'a>(_: &'a Ctx, _: &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>> {
    Box::pin(async { panic!("boom") })
}

fn leaky<'a>(_: &'a Ctx, _: &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>> {
    Box::pin(async { panic!("upstream said {TOKEN}") })
}

#[tokio::test]
async fn a_handler_that_crashes_is_reported_as_an_internal_error_and_the_server_carries_on() {
    let t = Harness::new();
    let (server, lines) = crashing(&t, boom);
    let client = connect_to(server).await;
    let result = call(&client, "repo_create", json!({ "name": "demo" })).await;
    assert!(is_error(&result));
    assert!(result.structured_content.is_none());
    assert_eq!(text(&result), "**Internal Error**\n\nboom");
    assert_match(&lines.lock().unwrap()[0], "^Handler crashed:\nboom");
    let after = call(&client, "repo_read", json!({ "repo": "g/r" })).await;
    assert_match(&text(&after), r"^\*\*Refused\*\*");
    client.cancel().await.ok();
}

#[tokio::test]
async fn a_crash_never_carries_the_token_in_the_result_or_in_the_log() {
    let t = Harness::with_token();
    let (server, lines) = crashing(&t, leaky);
    let client = connect_to(server).await;
    let result = call(&client, "repo_create", json!({ "name": "demo" })).await;
    assert!(is_error(&result));
    assert_eq!(text(&result), "**Internal Error**\n\nupstream said ***");
    let lines = lines.lock().unwrap().clone();
    assert!(lines.iter().all(|line| !line.contains(TOKEN)));
    assert_match(&lines[0], r"upstream said \*\*\*");
    client.cancel().await.ok();
}

#[test]
fn tool_result_renders_a_success_and_a_refusal() {
    let fields: Map<String, Value> = json!({ "id": 7, "text": "body" })
        .as_object()
        .cloned()
        .unwrap();
    let ok = tool_result("Done.", fields, true, "");
    assert!(!is_error(&ok));
    assert_eq!(text(&ok), "**Done.**\n\n- id: 7\n\n```text\nbody\n```");
    assert_eq!(
        ok.structured_content,
        Some(json!({ "id": 7, "text": "body" }))
    );
    let refused = tool_result("Boom", Map::new(), false, "why");
    assert_eq!(text(&refused), "**Boom**\n\nwhy");
    assert!(refused.structured_content.is_none());
    assert_eq!(
        text(&tool_result("", Map::new(), false, "why")),
        "**Refused**\n\nwhy"
    );
}
