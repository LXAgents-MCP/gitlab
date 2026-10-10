/*
 * The tool surface this server is meant to serve, pinned in one place.
 *
 * Adding, renaming or removing a tool is a change to a public surface and a release, so the
 * list is written out and not derived: the registry, protocol and stdio tests all compare
 * against it, and a tool that appears or vanishes without an edit here fails all three.
 *
 * Each tool is tagged with the kind of change it makes, and the registry test holds its
 * annotations to that kind:
 *
 *   read     only reads
 *   write    creates something; repeating the call repeats the effect
 *   update   changes something that exists; repeating it changes nothing more
 *   destroy  deletes, overwrites or merges
 */
export const SURFACE = {
  repo_create: "write",
  repo_read: "read",
  repo_update: "update",
  repo_delete: "destroy",
  repo_fork: "write",
  repo_search: "read",
  branch_create: "write",
  branch_read: "read",
  branch_list: "read",
  branch_delete: "destroy",
  branch_merge: "destroy",
  file_read: "read",
  file_create: "write",
  file_update: "update",
  file_delete: "destroy",
  directory_list: "read",
  commit_read: "read",
  commit_list: "read",
  commit_compare: "read",
  tag_check: "read",
  tag_get: "read",
  tag_get_all: "read",
  tag_delete: "destroy",
  release_create: "write",
  release_read: "read",
  release_list: "read",
  release_delete: "destroy",
  issue_open: "write",
  issue_read: "read",
  issue_comment: "write",
  issue_close: "update",
  issue_assign: "update",
  issue_label: "update",
  issue_lock: "update",
};

/** The hints each kind must carry. `openWorldHint` is true for all: every tool talks to GitLab. */
export const KIND_HINTS = {
  read: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  write: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: true },
  update: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
  destroy: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
};

/** Tools that do not take a `repo`: one creates a project, one searches across all of them. */
export const NO_REPO_ARGUMENT = ["repo_create", "repo_search"];

export const TOOL_NAMES = Object.keys(SURFACE).sort();
