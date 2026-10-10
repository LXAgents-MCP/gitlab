---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/gitlab — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-10-10

**Version `0.1.0`**, the first release. `package.json` and the lockfile carry it, and the release
log is `wiki/logs/0/1/0/CHANGELOG.md`. No git tag was made, by decision (D3 in
`tasks/forge-servers.md`).

**What it is.** An MCP server, stdio by default and HTTP with a bearer token, `lxagents-gitlab`, over the GitLab REST API at `gitlab.com`.
Plain JavaScript, Node ESM, no build step, two runtime dependencies: the MCP SDK and zod.

**Surface.** One tool per module in `src/tools/`. The full list is pinned in `test/surface.js` and
described in `wiki/reference/mcp-surface.md`: projects, branches, files and directories, commits,
tags and releases, issues, merge requests and pipelines. A tool count is deliberately not stated
here: it goes stale at the next module. `repo_create` was moved here from the `cli` server, where it
was `create_gitlab_repo`.

**Token.** The GitLab token: `MCP_AUTH_TOKEN` over stdio, read when a tool is called, or the bearer token
of each request over HTTP, which never reads it. `MCP_TRANSPORT`, `PORT` and `HOST` are the only other variables, and
the only flags are `--help` and `--version`.

**Verified.** The suite replaces `fetch` and reaches no host beyond loopback. Every request shape is
checked against GitLab's documentation and against mocks, and a read shape against public data on
`gitlab.com` where that was possible. The production-only dependency tree the image would install
serves every tool over a real stdio pipe.

**Not verified.** **Nothing has been exercised with a GitLab token:** the environment the work was
done in had none. The write tools are the ones to prove first, and `wiki/guides/smoke-test.md` is the
walk and the list of behaviours the documentation leaves open. A job's log has never been read, since
GitLab gives it only to a token. The `Dockerfile` has never been built: there was no Docker daemon.

**Not built.** No CI. No TLS, `Host` allow-list or rate limiting in the HTTP transport (decision
`http-token-transport.md`). No self-managed GitLab instances. Not published to npm (`private: true`). No git tag, no published image.

**Next.** The owner's smoke test, and whatever it finds. Anything it changes in a request is a patch
release with its own approval.
