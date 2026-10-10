---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/gitlab — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-10-10

**Version `0.2.0`.** `package.json` and the lockfile carry it, and the release log is
`wiki/logs/0/2/0/CHANGELOG.md`; the first release, `0.1.0`, is `wiki/logs/0/1/0/CHANGELOG.md`. 0.2.0 added the
HTTP transport and renamed the token variable to `MCP_AUTH_TOKEN`. No git tag was made, by decision (D3 in
`tasks/forge-servers.md`, D8 in `tasks/http-token-auth.md`).

**What it is.** An MCP server, stdio by default and HTTP with a bearer token, `lxagents-gitlab`, over the GitLab REST API at `gitlab.com`.
Plain JavaScript, Node ESM, no build step, two runtime dependencies: the MCP SDK and zod.

**Surface.** One tool per module in `src/tools/`. The full list is pinned in `test/surface.js` and
described in `wiki/reference/mcp-surface.md`: projects, branches, files and directories, commits,
tags and releases, issues, merge requests and pipelines. A tool count is deliberately not stated
here: it goes stale at the next module. `repo_create` was moved here from the `cli` server, where it
was `create_gitlab_repo`.

**Tokens.** Two. `GITLAB_API_KEY` is the GitLab token, read when a tool is called, on both transports.
`MCP_AUTH_TOKEN` is the access token HTTP requires of a client (32 characters or more; HTTP exits 1 without it) and
is never read over stdio or sent to GitLab. `MCP_TRANSPORT`, `PORT` and `HOST` are the only other variables, and the
only flags are `--help` and `--version`.

**Verified.** The suite (292 tests) replaces `fetch` and reaches no host beyond loopback. Every request shape is
checked against GitLab's documentation and against mocks, and a read shape against public data on
`gitlab.com` where that was possible. The production-only dependency tree the image would install
serves every tool over a real stdio pipe, and the HTTP transport is tested over real
loopback sockets, including concurrent requests with different tokens and a real process.

**Not verified.** **Nothing has been exercised with a GitLab token:** the environment the work was
done in had none. The write tools are the ones to prove first, and `wiki/guides/smoke-test.md` is the
walk and the list of behaviours the documentation leaves open. A job's log has never been read, since
GitLab gives it only to a token. The `Dockerfile` has never been built: there was no Docker daemon. The HTTP transport has not
been reached from another machine, behind a proxy, or with a real GitLab token.

**Not built.** No CI. No TLS, `Host` allow-list or rate limiting in the HTTP transport (decision
`http-token-transport.md`). No self-managed GitLab instances. Not published to npm (`private: true`). No git tag, no published image.

**Next.** The owner's smoke test, and whatever it finds. Anything it changes in a request is a patch
release with its own approval.
