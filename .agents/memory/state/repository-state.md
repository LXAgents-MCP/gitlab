---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/gitlab — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-10-10

**Unreleased.** `package.json` is `0.0.0`. The first release, `0.1.0`, is the last task of the
request recorded in `tasks/forge-servers.md`; until it lands this file describes a repository being
built, and the release task rewrites it.

**What it is.** A stdio MCP server, `lxagents-gitlab`, over the GitLab REST API at `gitlab.com`.
Plain JavaScript, Node ESM, no build step, two runtime dependencies: the MCP SDK and zod.

**Surface.** One tool per module in `src/tools/`, and the full list is pinned in
`test/surface.js`. `repo_create` was moved here from the `cli` server, where it was
`create_gitlab_repo`. The others are added by the tasks that follow it in the record. A tool count
is deliberately not stated here: it goes stale at the next module.

**Token.** `LXAGENTS_MCP_GITLAB_API_KEY`, read when a tool is called. No other variable is read,
and the only flags are `--help` and `--version`.

**Verified.** The suite replaces `fetch` and reaches no host beyond loopback. **Nothing has been
exercised with a GitLab token:** the environment the work was done in had none. Every request shape
is checked against GitLab's documentation and against mocks, and a read shape against public data on
`gitlab.com` where that was possible. A first smoke test is the owner's.

**Not built.** No CI. No HTTP transport (decision `stdio-only-transport.md`). No self-managed GitLab instances.
Not published to npm (`private: true`). A `Dockerfile` for a stdio image exists and has never been built.
