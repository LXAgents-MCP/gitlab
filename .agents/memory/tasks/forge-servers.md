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
| D13 | GitLab.com only, with a personal, project or group access token in `PRIVATE-TOKEN`. Declined: a configurable base URL for self-managed instances, which would be a second setting and a second trust decision. |

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
