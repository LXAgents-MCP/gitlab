//! The tool registry: one module per tool, listed once here.
//!
//! Adding a tool is a new file in this directory with `def()` (name, description, schemas and hints) and
//! `handle(ctx, args)`, resolving to an `Outcome` or a `Refused`, and one `tool!` line below. The file
//! name is the tool name: `repo_create.rs` is `repo_create`, and a test holds the two together in both
//! directions. `src/routes/server.rs` never names a tool, so the protocol layer cannot grow a per-tool
//! special case.
use std::sync::OnceLock;

use futures::future::BoxFuture;
use serde_json::Value;

use crate::models::args::Args;
use crate::models::outcome::Outcome;
use crate::models::schemas::Hints;
use crate::services::context::Ctx;
use crate::utils::errors::Refused;

pub mod prelude;

/// What a tool module declares: its name, what it is for, its schemas and its hints.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
    pub annotations: Hints,
}

/// A tool module's handler. Its arguments are checked by the handler itself, so they arrive untyped.
pub type Handler = for<'a> fn(&'a Ctx, &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>>;

/// A tool as the registry holds it.
#[derive(Clone)]
pub struct ToolSpec {
    pub def: ToolDef,
    pub handle: Handler,
}

impl ToolSpec {
    pub fn name(&self) -> &'static str {
        self.def.name
    }
}

macro_rules! tools {
    ($($module:ident),* $(,)?) => {
        $(pub mod $module;)*

        /// Every tool module, as specs, in file-name order.
        pub fn specs() -> Vec<ToolSpec> {
            vec![$({
                fn handle<'a>(ctx: &'a Ctx, args: &'a Args) -> BoxFuture<'a, Result<Outcome, Refused>> {
                    Box::pin($module::handle(ctx, args))
                }
                ToolSpec { def: $module::def(), handle }
            }),*]
        }

        /// The module names, which are the file names.
        pub const MODULES: &[&str] = &[$(stringify!($module)),*];
    };
}

tools!(
    branch_create,
    branch_delete,
    branch_list,
    branch_merge,
    branch_read,
    commit_compare,
    commit_list,
    commit_read,
    directory_list,
    file_create,
    file_delete,
    file_read,
    file_update,
    issue_assign,
    issue_close,
    issue_comment,
    issue_label,
    issue_lock,
    issue_open,
    issue_read,
    pr_approve,
    pr_close,
    pr_comment,
    pr_merge,
    pr_open,
    pr_read,
    pr_review_request,
    release_create,
    release_delete,
    release_list,
    release_read,
    repo_create,
    repo_delete,
    repo_fork,
    repo_read,
    repo_search,
    repo_update,
    tag_check,
    tag_delete,
    tag_get,
    tag_get_all,
    workflow_list,
    workflow_logs,
    workflow_run,
    workflow_status,
);

/// Hold specs by name, refusing two with one name: two tools answering to one name makes `tools/call`
/// dispatch arbitrary, so it fails loudly at start rather than silently at call time.
pub fn build_registry(specs: Vec<ToolSpec>) -> Result<Vec<ToolSpec>, String> {
    let mut seen: Vec<&'static str> = Vec::new();
    for spec in &specs {
        if spec.name().is_empty() {
            return Err("a tool has no 'name'.".to_string());
        }
        if seen.contains(&spec.name()) {
            return Err(format!("Duplicate tool name '{}'.", spec.name()));
        }
        seen.push(spec.name());
    }
    Ok(specs)
}

/// The registry, built once.
pub fn tools() -> &'static [ToolSpec] {
    static TOOLS: OnceLock<Vec<ToolSpec>> = OnceLock::new();
    TOOLS.get_or_init(|| build_registry(specs()).unwrap_or_else(|problem| panic!("{problem}")))
}

/// A tool by name.
pub fn get(name: &str) -> Option<&'static ToolSpec> {
    tools().iter().find(|spec| spec.name() == name)
}
