//! The MCP server: protocol and tool dispatch only.
//!
//! A fresh instance serves each connection. The tool surface is not written here - it is the registry in
//! `src/controllers/`.
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Once};

use futures::FutureExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};

use crate::config::version::{SERVER_NAME, VERSION};
use crate::controllers::{ToolDef, ToolSpec, tools};
use crate::services::auth::token_to_scrub;
use crate::services::context::Ctx;
use crate::utils::log::log;
use crate::utils::redact::redact;
use crate::utils::render::render_payload;

/// What every client reads at `initialize`, before it has called a single tool.
pub const INSTRUCTIONS: &str = "GitLab.com through its REST API: projects (called repositories here), branches, files, \
     commits, tags, releases, issues, merge requests (called pull requests here) and CI/CD pipelines (called workflows \
     here). Every tool needs GITLAB_API_KEY in this server's environment. Name a project by its full path, \
     'group/project' or 'group/subgroup/project', or by its numeric id. Tools that delete or merge are destructive: \
     confirm with the owner before calling them.";

/// Where a crash is written: stderr in production, a buffer in a test.
pub type Logger = Arc<dyn Fn(&str) + Send + Sync>;

/// Shape a handler's outcome into a `tools/call` result.
///
/// `isError` is how a client learns a call failed. The text block is what the model reads; the fields
/// also ride along in `structuredContent`, except on a refusal or a crash, which have no fields to
/// report and whose empty object a client would reject against the output schema.
pub fn tool_result(
    summary: &str,
    fields: Map<String, Value>,
    ok: bool,
    message: &str,
) -> CallToolResult {
    let text = if message.is_empty() {
        render_payload(summary, &fields)
    } else {
        format!(
            "**{}**\n\n{message}",
            if summary.is_empty() {
                "Refused"
            } else {
                summary
            }
        )
    };
    let content = vec![ContentBlock::text(text)];
    let mut result = if ok {
        CallToolResult::success(content)
    } else {
        CallToolResult::error(content)
    };
    if message.is_empty() {
        result.structured_content = Some(Value::Object(fields));
    }
    result
}

/// The text of a panic, as a handler that crashed left it.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "a handler panicked".to_string()
    }
}

tokio::task_local! {
    /// Set while a handler runs, so the panic hook knows a crash is reported by `call_tool`.
    static IN_TOOL: ();
}

/// Run one tool call, turning every failure into a result the client can read.
pub async fn call_tool(
    ctx: &Ctx,
    spec: &ToolSpec,
    args: &Map<String, Value>,
    logger: &Logger,
) -> CallToolResult {
    match IN_TOOL
        .scope(
            (),
            AssertUnwindSafe((spec.handle)(ctx, args)).catch_unwind(),
        )
        .await
    {
        Ok(Ok(outcome)) => tool_result(&outcome.summary, outcome.fields.0, outcome.ok, ""),
        Ok(Err(refused)) => tool_result("Refused", Map::new(), false, &refused.0),
        Err(panic) => {
            // A crash is text nobody wrote for a reader, so it is scrubbed like any other: the token
            // must not reach the log or the client whatever put it into the message.
            let secret = token_to_scrub(ctx);
            let text = redact(&panic_text(panic.as_ref()), &secret);
            logger(&format!("Handler crashed:\n{text}"));
            tool_result("Internal Error", Map::new(), false, &text)
        }
    }
}

/// The panic hook a server installs: Rust's default one prints the panic, which may carry the token, so
/// it is replaced by one that prints nothing. `call_tool` logs a crash itself, redacted.
pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Only a panic outside a tool call is printed, as Rust would; inside one, `call_tool` reports it.
            if IN_TOOL.try_with(|_| ()).is_err() {
                previous(info);
            }
        }));
    });
}

fn as_object(value: &Value) -> Arc<Map<String, Value>> {
    Arc::new(value.as_object().cloned().unwrap_or_default())
}

/// A tool as `tools/list` advertises it.
pub fn advertise(def: &ToolDef) -> Tool {
    let hints = def.annotations;
    Tool::new(def.name, def.description, as_object(&def.input_schema))
        .with_raw_output_schema(as_object(&def.output_schema))
        .with_annotations(
            ToolAnnotations::new()
                .read_only(hints.read_only)
                .destructive(hints.destructive)
                .idempotent(hints.idempotent)
                .open_world(hints.open_world),
        )
}

/// The server one connection talks to.
#[derive(Clone)]
pub struct GitlabServer {
    pub ctx: Ctx,
    pub tools: Arc<Vec<ToolSpec>>,
    pub logger: Logger,
}

impl GitlabServer {
    /// A server over the registry, logging to stderr.
    pub fn new(ctx: Ctx) -> Self {
        GitlabServer {
            ctx,
            tools: Arc::new(tools().to_vec()),
            logger: Arc::new(log),
        }
    }
}

impl ServerHandler for GitlabServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_server_info(Implementation::new(SERVER_NAME, VERSION))
        .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(
            self.tools.iter().map(|spec| advertise(&spec.def)).collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let Some(spec) = self.tools.iter().find(|spec| spec.name() == request.name) else {
            let text = format!("Tool {} not found", request.name);
            return Ok(CallToolResult::error(vec![ContentBlock::text(text)]).into());
        };
        let args = request.arguments.unwrap_or_default();
        Ok(call_tool(&self.ctx, spec, &args, &self.logger).await.into())
    }
}
