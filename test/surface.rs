//! The tool surface this server is meant to serve, pinned in one place.
//!
//! Adding, renaming or removing a tool is a change to a public surface and a release, so the list is
//! written out and not derived: the registry, protocol and stdio tests all compare against it.
//!
//! Each tool is tagged with the kind of change it makes, and the registry test holds its annotations to
//! that kind: read only reads; write creates something, and repeating it repeats the effect; update
//! changes something that exists, and repeating it changes nothing more; destroy deletes, overwrites or
//! merges.
#![allow(dead_code)]

use serde_json::{Value, json};

pub const SURFACE: &[(&str, &str)] = &[
    ("repo_create", "write"),
    ("repo_read", "read"),
    ("repo_update", "update"),
    ("repo_delete", "destroy"),
    ("repo_fork", "write"),
    ("repo_search", "read"),
    ("branch_create", "write"),
    ("branch_read", "read"),
    ("branch_list", "read"),
    ("branch_delete", "destroy"),
    ("branch_merge", "destroy"),
    ("file_read", "read"),
    ("file_create", "write"),
    ("file_update", "update"),
    ("file_delete", "destroy"),
    ("directory_list", "read"),
    ("commit_read", "read"),
    ("commit_list", "read"),
    ("commit_compare", "read"),
    ("tag_check", "read"),
    ("tag_get", "read"),
    ("tag_get_all", "read"),
    ("tag_delete", "destroy"),
    ("release_create", "write"),
    ("release_read", "read"),
    ("release_list", "read"),
    ("release_delete", "destroy"),
    ("issue_open", "write"),
    ("issue_read", "read"),
    ("issue_comment", "write"),
    ("issue_close", "update"),
    ("issue_assign", "update"),
    ("issue_label", "update"),
    ("issue_lock", "update"),
    ("pr_open", "write"),
    ("pr_read", "read"),
    ("pr_comment", "write"),
    ("pr_close", "update"),
    ("pr_merge", "destroy"),
    ("pr_review_request", "update"),
    ("pr_approve", "update"),
    ("workflow_list", "read"),
    ("workflow_run", "write"),
    ("workflow_status", "read"),
    ("workflow_logs", "read"),
];

/// The kind of a tool.
pub fn kind(name: &str) -> &'static str {
    SURFACE
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
        .unwrap_or_else(|| panic!("{name} is not in SURFACE"))
}

/// The hints each kind must carry. `openWorldHint` is true for all: every tool talks to GitLab.
pub fn kind_hints(kind: &str) -> Value {
    let (read_only, destructive, idempotent) = match kind {
        "read" => (true, false, true),
        "write" => (false, false, false),
        "update" => (false, false, true),
        "destroy" => (false, true, true),
        other => panic!("no kind {other}"),
    };
    json!({ "readOnlyHint": read_only, "destructiveHint": destructive, "idempotentHint": idempotent, "openWorldHint": true })
}

/// Tools that do not take a `repo`: one creates a project, one searches across all of them.
pub const NO_REPO_ARGUMENT: &[&str] = &["repo_create", "repo_search"];

pub fn tool_names() -> Vec<String> {
    let mut names: Vec<String> = SURFACE.iter().map(|(n, _)| n.to_string()).collect();
    names.sort();
    names
}
