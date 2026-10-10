/**
 * What GitLab says about whether a merge request can be merged, in one place.
 *
 * GitLab works it out in the background after a push, so a merge request read straight away may
 * still be "checking". `detailed_merge_status` replaced the older `merge_status`, and an answer from
 * an older GitLab may carry only the latter.
 */

const PENDING = new Set(["checking", "unchecked", "preparing", "approvals_syncing"]);

// What each status that is neither ready nor pending means, said so the model can act on it.
const REASONS = {
  conflict: "the branches conflict",
  need_rebase: "the source branch has to be rebased first",
  ci_must_pass: "its pipeline has to pass first",
  ci_still_running: "its pipeline is still running",
  commits_status: "its commits do not pass the project's checks",
  not_approved: "it needs an approval first",
  requested_changes: "a reviewer requested changes",
  discussions_not_resolved: "its discussions are not resolved",
  draft_status: "it is a draft",
  status_checks_must_pass: "external status checks have to pass first",
  merge_request_blocked: "another merge request blocks it",
  jira_association_missing: "it has to mention a Jira issue first",
  security_policy_violations: "a security policy blocks it",
  security_policy_pipeline_check: "a security policy needs its pipeline first",
  locked_paths: "it changes paths that are locked",
  locked_lfs_files: "it changes LFS files that are locked",
  title_regex: "its title does not match the project's pattern",
  merge_time: "it is set to merge only after a given time",
  not_open: "it is not open",
};

/**
 * @param {any} mr A merge request as GitLab returns it.
 * @returns {{ status: string, ready: boolean, pending: boolean, reason: string }}
 */
export function mergeability(mr) {
  const detailed = typeof mr?.detailed_merge_status === "string" ? mr.detailed_merge_status : undefined;
  if (detailed !== undefined) {
    return {
      status: detailed,
      ready: detailed === "mergeable",
      pending: PENDING.has(detailed),
      reason: REASONS[detailed] ?? `GitLab reports it as ${detailed}`,
    };
  }

  const legacy = typeof mr?.merge_status === "string" ? mr.merge_status : "unchecked";
  return {
    status: legacy,
    ready: legacy === "can_be_merged",
    pending: legacy === "unchecked" || legacy === "checking" || legacy === "cannot_be_merged_recheck",
    reason: legacy === "cannot_be_merged" ? "the branches conflict or it cannot be merged" : `GitLab reports it as ${legacy}`,
  };
}
