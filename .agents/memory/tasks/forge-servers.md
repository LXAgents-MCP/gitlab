---
name: memory-tasks-forge-servers
description: Building this GitLab MCP server — moving create_gitlab_repo out of cli and growing it to the 45-tool GitLab surface; released as 0.1.0.
---

# Forge servers — GitLab

## 2026-10-10 — in progress

**Goal.** An LLM can work with GitLab — projects, branches, files, commits, tags, releases,
issues, merge requests and CI/CD pipelines — through a dedicated MCP server, and the host-shell
server (`cli`) no longer carries any forge code.

**Objective.** `node src/index.js` serves 45 tools over stdio, named as the owner listed them.
`repo_create` is the `create_gitlab_repo` tool moved out of `cli` 2.1.0, renamed, with the same
arguments and behaviour. The token is read from `LXAGENTS_MCP_GITLAB_API_KEY` at call time and
never appears in an argument, a log line or an error. `npm test` is green and no test reaches a
host beyond this machine. The repository has the structure of `shared-instruction`, without
`content/`.

**Detail.** One request spans three repositories — `LXAgents-MCP/github`, `LXAgents-MCP/gitlab`
and `LXAgents-MCP/cli` — built in that order, each with its own record under this slug. This
repository is the GitHub server's counterpart: the same 45 tool names, with GitLab's own words
underneath (a repository is a project, a pull request is a merge request, a workflow is a
pipeline). `cli` drops its two forge tools last, so `repo_create` exists elsewhere before it
disappears there. This repository consumes the shared instruction set through the connector and
carries none of it. Stdio only, Node 20 or newer, no build step; the runtime dependencies are the
MCP SDK and zod. GitLab means gitlab.com. The moved code comes from `cli` commits `827b25a`,
`9babff7` and `5f9b64f`; its history is not transplanted.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/forge-servers-plan` | |
| 2 | Server core and `repo_create` | `feat/server-core` | |
| 3 | Agent instruction set | `docs/agents-setup` | |
| 4 | Human documentation | `docs/wiki` | |
| 5 | Repository tools | `feat/repo-tools` | |
| 6 | Branch tools | `feat/branch-tools` | |
| 7 | File and directory tools | `feat/file-tools` | |
| 8 | Commit tools | `feat/commit-tools` | |
| 9 | Tag and release tools | `feat/tag-release-tools` | |
| 10 | Issue tools | `feat/issue-tools` | |
| 11 | Merge request tools | `feat/pr-tools` | |
| 12 | Pipeline tools | `feat/workflow-tools` | |
| 13 | Container image | `build/docker` | |
| 14 | Release | `release/0.1.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by
task 14, once the pull requests exist.

## Decisions the owner approved with the plan

Where a choice was needed, the owner's standing instruction is to offer the options with a
recommendation and use the recommended one. Each row names the option taken and the one declined.

| # | Decision |
|---|---|
| D1 | Branches follow `branching_strategy`: `chore/forge-servers-plan`, `feat/…`, `release/{version}`. The `claude/` prefix is not used, by the owner's instruction. Declined: one branch per repository. |
| D2 | Stdio only, as in `cli`. The server carries a token that can delete projects, so an HTTP listener would let any holder of its bearer token act as the owner. Declined: stdio plus HTTP as in `shared-instruction`. HTTP would be its own approved task with a security record. |
| D3 | First release is `0.1.0`: a 45-tool surface whose write tools are exercised only against mocks. No git tag is created. |
| D4 | The package is `private: true`, as in `cli`. No npm publish. |
| D5 | Every tool except `repo_create` and `repo_search` takes one `repo` argument: the project's full path, `group/subgroup/project`, or its numeric id, parsed and encoded in one place. Declined: separate namespace and project arguments. |
| D6 | The 45 tools are exactly the owner's list. Not added: `issue_list`, `pr_list`, `tag_create` (a tag comes from `release_create`), `issue_reopen`, inline review comments. |
| D7 | Tasks run strictly in order, with no checkpoint and no parallel agents. |
| D8 | Pull requests are opened one per branch and merged in order 1…n by rebase, each branch deleted as it merges — the owner gave both permissions. No tag and no npm publish. |
| D9 | Standing instruction: where a choice is needed, use the recommended option and add it to this table. |
| D10 | The tool names are the owner's in both servers, and GitLab's words sit underneath: `repo_` acts on a project, `pr_` on a merge request (its `number` is the project-scoped `iid`), `workflow_` on a pipeline — `workflow_list` lists recent pipelines, `workflow_run` creates one, `workflow_status` reads one, `workflow_logs` reads its jobs' logs. Declined: GitLab-specific names (`mr_`, `pipeline_`), which would make the two servers' tools differ. |
| D11 | GitLab has no direct branch merge, so `branch_merge` opens a merge request from `head` into `base` and accepts it; the merged request stays as the record, and if it cannot be merged the request it opened is closed again. Declined: a local git merge, which the server cannot do. |
| D12 | Where GitLab has no counterpart of a GitHub argument — draft releases, generated release notes, close and lock reasons — the argument is absent and the documentation says so. Declined: accepting it and ignoring it. |
| D13 | GitLab.com only, with a personal, project or group access token. It is sent as `Authorization: Bearer`, which GitLab accepts for all three: the runtime drops that header when a redirect changes host, and would forward GitLab's own `PRIVATE-TOKEN`. Declined: `PRIVATE-TOKEN`, which `cli` used, and a configurable base URL for self-managed instances, which would be a second setting and a second trust decision. |

## Out of scope

HTTP transport, GitLab self-managed instances, CI configuration in this repository, live
integration tests, npm publishing, git tags.

## Task entries

### Task 1 — chore/forge-servers-plan

Landed: this record, `.agents/index/memory-index.md`, a minimal `.agents/index/root-index.md`
so the memory index has a parent, and `.gitignore`. The repository had no `.gitignore`; its
`/.agents/plans/` line keeps the untracked working plan out of history, and the owner approved
adding it with the plan. The root index lists only the memory index for now; task 3 completes
it. Nothing else changes in this task, and no code exists yet. The `PR` column is filled by task 14.

### Task 2 — feat/server-core

Landed. `node src/index.js` is a stdio MCP server named `lxagents-gitlab` that serves one tool,
`repo_create`, the `create_gitlab_repo` tool moved out of `cli` 2.1.0 with its arguments, defaults
and refusal wording unchanged. `package.json` is `private`, at `0.0.0` until the release task, with
the same two runtime dependencies as `cli`; the lockfile started from the GitHub server's, which
started from `cli`'s, so the audited dependency set is the same.

Shape. The modules that do not know the forge — `args.js`, `errors.js`, `log.js`, `render.js`,
`schemas.js`, `text.js`, the registry and the entry point — are the GitHub server's, with the
names changed. `src/api.js` is GitLab's own. It is the only place a URL is built: `parseRepo` takes
a project's full path or its numeric id, and `refPath` and `filePath` write a branch name or a file
path as the single URL segment GitLab reads, the slashes encoded, where GitHub keeps them literal.
`gitlab()` reads the token per call and turns every failure into one message that names the next
step; a tool may word the statuses it knows better through `explain`, which is how `repo_create`
keeps its old messages. `apiDetail` reads the four shapes GitLab answers in: a message, an object of
field errors, a list, and an OAuth-style `error`. A list says whether another page follows from the
`x-next-page` header, which GitLab sets and GitHub does not.

One change from `cli`, recorded as D13: the token goes in `Authorization: Bearer`, not
`PRIVATE-TOKEN`. GitLab accepts a personal, project or group access token either way, and the runtime
drops `Authorization` — and no custom header — when a redirect changes host, which GitLab uses to
move a project. A test over two loopback servers holds the client to it. Nothing a caller can see
changes.

Pinned. `test/surface.js` is the one place the tool list is written out, each tool tagged read,
write, update or destroy, and the registry, protocol and stdio tests compare against it. A crash is
scrubbed like a refusal from the start: the unexpected-exception path redacts the token from both
the log line and the text returned to the client.

Tests: the runner reports 105 passing and none failing. Two of those are the support files under
`test/`, which `node --test` also executes and counts. No test reaches a host beyond this machine;
the one redirect test talks to two servers on loopback.

Left for later tasks: the other 44 tools (tasks 5-12), the instruction set and wiki (3, 4), the
image (13) and the release (14).

### Task 3 — docs/agents-setup

Landed. `AGENTS.md` is the entry point of a consuming repository: it declares the shared tools this
repository uses (`automation` and the four mandatory ones, plus `pull_request_template`), carries
the three gates and the discovery gate inline, says how to use the connector and what to say when it
does not resolve, and adds four trigger rows of its own. `.claude/CLAUDE.md` imports it so Claude
Code and every other agent read one file. The local set: two rules — `gitlab-repository-rules` and
`tool-surface` — the agent wiki's `repository-map` and `security-boundaries` pages, a state file, the
stdio-only decision, and three indexes under a root index whose override table is empty. It is the
GitHub server's set with the forge changed; the sentences that differ are the ones about projects, the
full-path argument and self-managed instances.

Names. The local repository rule is `gitlab-repository-rules`, not `repository-rules`, because the
shared set already has a file of that name and a local file with it would read as an undeclared
override.

Forward references. Four links in the agent wiki point at human wiki pages that task 4 writes
(`architecture`, `mcp-surface`, `env`, `security-model`). They resolve when task 4 lands; a link
check over `AGENTS.md` and `.agents/` finds no other unresolved link. The project wiki index and
the logs index are created with the pages they list, in tasks 4 and 14.

The security page is written from the code as it stands and follows the shape the shared security
creator gives. It will gain a row for each destructive tool as those tools land.

Nothing here copies the shared set. Tests unchanged: 105 passing, none failing.

### Task 4 — docs/wiki

Landed. `README.md` is an overview that points at the wiki and states no tool count, and seven wiki
pages sit under a project wiki index: overview, architecture, MCP surface, setup, environment,
connect-a-client and the security model. `mcp-surface.md` and `env.md` carry the tables that grow:
each tool group adds its rows to both, and to the README and security model where it changes what
they say, in the same commit as the tools.

It is the GitHub server's documentation with the forge changed, and the pages say where the words
differ: the overview has a table of the same tool prefixes in each forge's words (a repository is a
project, a pull request a merge request, a workflow a pipeline); the token section is about scopes,
roles and project or group access tokens and not about a fine-grained token's permissions; the
security model says why the token travels as `Authorization` and not in GitLab's own header.

Facts once. The surfaces tables task 3 put in the agent security page now live in the human
security model; the agent page keeps the isolation rule, the before-you-change table, escalation
and what is not a finding. Claims about the `repo_delete` name-confirmation guard are left out of
the README and the security model until task 5 adds the tool, so no page describes code that does
not exist yet.

The pages that say nothing has been run live say it of a token and not of the host: unlike the
GitHub server's environment, this one could read public data on `gitlab.com`, so a read shape can be
compared with a real answer where the group has one.

A link check over `README.md`, `wiki/`, `AGENTS.md` and `.agents/` finds no unresolved link. The
human pages say nothing about `.agents/`. Tests unchanged: 105 passing, none failing.

### Task 5 — feat/repo-tools

Landed. `repo_read`, `repo_update`, `repo_delete`, `repo_fork` and `repo_search`, pinned in
`test/surface.js` and documented in the surface page, the token table, the README and the security
model.

Checked against GitLab's REST documentation before writing: a project is read, updated and deleted at
`/projects/:id`, with a path written as one encoded segment; an update takes `name`, `path`,
`description`, `visibility`, `default_branch` and `topics`, while archiving and unarchiving are their
own idempotent endpoints that need the Owner role; a delete marks the project for deletion and it is
removed at the end of a retention period; a fork takes `namespace_path`, `name` and `path`, and the
page says the copy is asynchronous and read through `import_status`; a search is a case-insensitive
substring match of every word over name, path and description, with `order_by` and `sort`. The pages
state no status code for delete or fork, so the tools accept any 2xx. The Update table lists
`issues_enabled` as deprecated in favour of `issues_access_level`, which is what `has_issues` sends;
the excerpt read here did not show that field itself, so it is on the live-check list.

Read shapes were compared with real data: a public project and a project search on `gitlab.com` pass
through `projectFields` and the output schema. An anonymous answer lacks `archived`, the open issue
count and more, which a token's answer has, so the mapper treats them as absent and not as an error.

Shape. `projectShape`, `projectFields` and `describeProject` in `src/shapes.js` are what read, update
and fork share, as the repository shape is in the GitHub server, and `expectBody` turns a 2xx answer of
the wrong shape into a refusal. `isNamespacePath` joins `SEGMENT` in `src/api.js` so that `repo_create`
and `repo_fork` check a group the same way.

Where GitLab differs, the tool says so. `repo_update` has `path` as well as `name`, because the name
is not the address; takes `visibility` where GitHub takes `private`; and has no homepage. It
unarchives before it changes settings and archives after, because an archived project is read-only,
addresses the later requests by id, since a new path moves the project, and says what had already
happened if a later step is refused. `repo_delete` answers `accepted` and not `deleted`: GitLab may
only schedule the deletion, and the summary says so. Its guard is the project's full path, resolved
through one read when the project was named by number. `repo_search` has no qualifier syntax to offer
and reports GitLab's `x-total` when it sends one.

The guard. `repo_delete` refuses unless `confirm` repeats the full path, ignoring case, and a test
shows nothing is sent otherwise. The README and the security model carry that claim now that the tool
exists; task 4 left it out until then.

Tests: 128 passing, none failing.

### Task 6 — feat/branch-tools

Landed. `branch_create`, `branch_read`, `branch_list`, `branch_delete` and `branch_merge`, pinned in
`test/surface.js` and documented in the surface page and the token table.

Checked against GitLab's REST documentation: a branch is read and deleted at
`/repository/branches/:branch` with the name URL-encoded, listed with `search` (a substring, `^` for
a start, `$` for an end) or `regex`, and created with `branch` and `ref`, which the page's example
sends as query parameters; deletion answers 204 and refuses the default and protected branches. A
request against `gitlab.com` confirmed that a branch name with a slash must be written `%2F` — the
literal slash is a 404 — and the compare endpoint's shape (`commits`, `diffs`, `compare_same_ref`).
Branches are the one group where the paths are encoded differently from the GitHub server's, which
`refPath` already does.

Where GitLab differs. The list endpoint has no protected filter, so `branch_list` takes `search`
where the GitHub server takes `protected`, and reports `default` and `merged` as well. `branch_create`
looks up the default branch when no source is given, because `ref` is required. There is no direct
merge: `branch_merge` compares the two branches first, so a head the base already holds is answered
as up to date and opens nothing; otherwise it opens a merge request from head into base, waits for
GitLab to say whether it can be merged (it works that out in the background, up to ten reads a
second apart), and accepts it with the head's sha so a branch that moved is refused. If it cannot be
merged, or the merge is refused (401, 405, 409, 422 as the merge page lists them), the request it
opened is closed again and the refusal says so — or says it is still open when closing failed.
`src/merge-status.js` turns `detailed_merge_status` into ready, pending or a reason in words, and
falls back to the older `merge_status`; merge request tools will use it too.

Tests: 151 passing, none failing.
