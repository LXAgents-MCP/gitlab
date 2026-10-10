# Overview

`LXAgents-MCP/gitlab` is an MCP server, `lxagents-gitlab`, that lets an LLM work with GitLab.com
through its REST API: read and change projects, branches, files, commits, tags, releases, issues,
merge requests and CI/CD pipelines. It speaks stdio, where the client starts it as a
subprocess, or HTTP, where each request carries the server's access token as a bearer token.

## Why it is its own server

GitLab access first lived in the host-shell server, `cli`: one tool, `create_gitlab_repo`, beside
`run_command`, `read_file` and `list_directory`. That put a credentialed API client in the same
process as a tool that runs arbitrary commands, and `cli`'s own documentation had to explain that
its `--confine` and `--allow` flags bound the shell tools and do not apply to the GitLab one. Two
trust boundaries shared one process.

Split, each server has one job and one boundary. `cli` runs commands and reads files on the host
and makes no network call of its own. This server calls `gitlab.com`, reads no local file and
runs no command. `repo_create` moved here from `cli`, renamed from `create_gitlab_repo`, with its
arguments and behaviour unchanged. GitHub is the same arrangement in its own repository,
`LXAgents-MCP/github`.

## The same tools, in GitLab's words

This server and the GitHub one expose the same tool names, so an agent that knows one knows the
other. Underneath, GitLab has its own words, and the tools use them:

| Tool prefix | GitHub's word | GitLab's word |
|---|---|---|
| `repo_` | repository | project, named by its full path (`group/subgroup/project`) or numeric id |
| `pr_` | pull request | merge request, numbered by its project-scoped `iid` |
| `workflow_` | Actions workflow | CI/CD pipeline |

Where GitLab has no counterpart of something GitHub has, the argument is absent, and
[MCP surface](../reference/mcp-surface.md) says so.

## What it serves

**Tools, and nothing else.** No prompts and no resources. The server covers the families of
operation an agent needs on a project, each with its own tool prefix:

| Prefix | Acts on |
|---|---|
| `repo_` | Projects: create, read, update, delete, fork, search. |
| `branch_` | Branches: create, read, list, delete, merge. |
| `file_`, `directory_list` | Files and directories, changed by committing through the API. |
| `commit_` | History: read, list, compare. |
| `tag_`, `release_` | Tags and releases. |
| `issue_` | Issues: open, read, comment, close, assign, label, lock. |
| `pr_` | Merge requests: open, read, comment, close, merge, request a review, approve. |
| `workflow_` | CI/CD pipelines: list, run, status, logs. |

Which of these exist in a given version is listed, tool by tool, in
[MCP surface](../reference/mcp-surface.md).

## Where the tokens come from

A tool that took the GitLab token as an argument would put it in a prompt and a transcript. So the server reads
it, as `GITLAB_API_KEY`, from its own environment, set by the client's configuration over stdio, when a tool is
called rather than at startup, so a server without one still starts and says what is missing when asked. Over
HTTP a second secret appears: the access token, `MCP_AUTH_TOKEN`, which a client sends as
`Authorization: Bearer <token>` to be allowed in. It never reaches GitLab, and HTTP will not start without it.
See [Environment](../environments/env.md) and [Set up HTTP with two tokens](../guides/setup-http-token.md).

## What it is not

* **Not a git client.** It does not clone or push. File tools commit through the API.
* **Not self-managed GitLab.** The API base is `https://gitlab.com/api/v4`, a constant; supporting
  another host would need a second setting, which is a decision for the owner.
* **Not a hosted service.** The HTTP transport is yours to run, it listens on every interface
  by default, it will not start without an access token, and it needs TLS in front before it faces a network. See
  [Security model](../security/security-model.md).

## Related pages

* [Architecture](architecture.md) — how a call becomes a request and a result.
* [MCP surface](../reference/mcp-surface.md) — every tool.
* [Security model](../security/security-model.md)
