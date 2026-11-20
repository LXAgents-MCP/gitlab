//! What GitLab says about whether a merge request can be merged, in one place.
//!
//! GitLab works it out in the background after a push, so a merge request read straight away may still be
//! "checking". `detailed_merge_status` replaced the older `merge_status`, and an answer from an older
//! GitLab may carry only the latter.
use serde_json::Value;

const PENDING: &[&str] = &["checking", "unchecked", "preparing", "approvals_syncing"];

// What each status that is neither ready nor pending means, said so the model can act on it.
const REASONS: &[(&str, &str)] = &[
    ("conflict", "the branches conflict"),
    ("need_rebase", "the source branch has to be rebased first"),
    ("ci_must_pass", "its pipeline has to pass first"),
    ("ci_still_running", "its pipeline is still running"),
    (
        "commits_status",
        "its commits do not pass the project's checks",
    ),
    ("not_approved", "it needs an approval first"),
    ("requested_changes", "a reviewer requested changes"),
    (
        "discussions_not_resolved",
        "its discussions are not resolved",
    ),
    ("draft_status", "it is a draft"),
    (
        "status_checks_must_pass",
        "external status checks have to pass first",
    ),
    ("merge_request_blocked", "another merge request blocks it"),
    (
        "jira_association_missing",
        "it has to mention a Jira issue first",
    ),
    ("security_policy_violations", "a security policy blocks it"),
    (
        "security_policy_pipeline_check",
        "a security policy needs its pipeline first",
    ),
    ("locked_paths", "it changes paths that are locked"),
    ("locked_lfs_files", "it changes LFS files that are locked"),
    (
        "title_regex",
        "its title does not match the project's pattern",
    ),
    ("merge_time", "it is set to merge only after a given time"),
    ("not_open", "it is not open"),
];

/// Whether a merge request can be merged, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mergeability {
    pub status: String,
    pub ready: bool,
    pub pending: bool,
    pub reason: String,
}

/// Read a merge request as GitLab returns it.
pub fn mergeability(mr: &Value) -> Mergeability {
    if let Some(detailed) = mr.get("detailed_merge_status").and_then(Value::as_str) {
        let reason = REASONS
            .iter()
            .find(|(status, _)| *status == detailed)
            .map(|(_, reason)| reason.to_string())
            .unwrap_or_else(|| format!("GitLab reports it as {detailed}"));
        return Mergeability {
            status: detailed.to_string(),
            ready: detailed == "mergeable",
            pending: PENDING.contains(&detailed),
            reason,
        };
    }
    let legacy = mr
        .get("merge_status")
        .and_then(Value::as_str)
        .unwrap_or("unchecked");
    Mergeability {
        status: legacy.to_string(),
        ready: legacy == "can_be_merged",
        pending: matches!(
            legacy,
            "unchecked" | "checking" | "cannot_be_merged_recheck"
        ),
        reason: if legacy == "cannot_be_merged" {
            "the branches conflict or it cannot be merged".to_string()
        } else {
            format!("GitLab reports it as {legacy}")
        },
    }
}
