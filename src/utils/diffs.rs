//! GitLab's file diffs as this server reports them.
//!
//! A diff answer is a list of files, each with its hunks as text and no counts, so the counts are read off
//! the hunks. GitLab starts a file's text at its first `@@`, with no `---` or `+++` header.
use serde_json::{Value, json};

use crate::utils::js::{at, js_string, truthy};

/// What a diff file's change was.
pub fn file_status(file: &Value) -> &'static str {
    if truthy(at(file, "new_file")) {
        "added"
    } else if truthy(at(file, "deleted_file")) {
        "removed"
    } else if truthy(at(file, "renamed_file")) {
        "renamed"
    } else {
        "modified"
    }
}

/// Lines added and removed, counted from the hunks.
pub fn count_lines(diff: &Value) -> (i64, i64) {
    let (mut additions, mut deletions) = (0, 0);
    if let Value::String(text) = diff {
        for line in text.split('\n') {
            if line.starts_with('+') {
                additions += 1;
            } else if line.starts_with('-') {
                deletions += 1;
            }
        }
    }
    (additions, deletions)
}

/// A changed file as a result lists it: `filename`, `status`, `additions`, `deletions`.
pub fn summarize_file(file: &Value) -> Value {
    let name = match (at(file, "new_path"), at(file, "old_path")) {
        (Value::Null, Value::Null) => Value::String(String::new()),
        (Value::Null, old) => old.clone(),
        (new, _) => new.clone(),
    };
    let (additions, deletions) = count_lines(at(file, "diff"));
    json!({ "filename": name, "status": file_status(file), "additions": additions, "deletions": deletions })
}

/// The files of a diff as one unified diff, with the headers GitLab leaves out.
pub fn unified_diff(files: &[Value]) -> String {
    let mut out = String::new();
    for file in files {
        let old = js_string(at(file, "old_path"));
        let new = js_string(at(file, "new_path"));
        let from = if truthy(at(file, "new_file")) {
            "/dev/null".to_string()
        } else {
            format!("a/{old}")
        };
        let to = if truthy(at(file, "deleted_file")) {
            "/dev/null".to_string()
        } else {
            format!("b/{new}")
        };
        let diff = at(file, "diff");
        let body = if truthy(diff) {
            js_string(diff)
        } else if truthy(at(file, "too_large")) || truthy(at(file, "collapsed")) {
            "(GitLab left this file's diff out because it is too large)\n".to_string()
        } else {
            String::new()
        };
        out.push_str(&format!(
            "diff --git a/{old} b/{new}\n--- {from}\n+++ {to}\n{body}"
        ));
    }
    out
}
