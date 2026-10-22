---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/gitlab — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-10-10

**Version `0.4.0`.** `package.json` and the lockfile carry it, and the release log is
`wiki/logs/0/4/0/CHANGELOG.md`; earlier releases are `0/3/0`, `0/2/0` and `0/1/0`. 0.4.0 moved the HTTP defaults to
`shared-instruction`'s, port 3000 on every interface. No git tag was made, by decision (D3 in
`tasks/forge-servers.md`, D8 in `tasks/http-token-auth.md`, T7 in `tasks/two-token-auth.md`, H3 in
`tasks/http-defaults.md`).

**What it is.** An MCP server, stdio by default and HTTP behind an access token, `lxagents-gitlab`, over the GitLab REST API at `gitlab.com`.
Plain JavaScript, Node ESM, no build step, two runtime dependencies: the MCP SDK and zod.

**Surface.** One tool per module in `src/controllers/`. The full list is pinned in `test/surface.js` and
described in `wiki/reference/mcp-surface.md`: projects, branches, files and directories, commits,
tags and releases, issues, merge requests and pipelines. A tool count is deliberately not stated
here: it goes stale at the next module. `repo_create` was moved here from the `cli` server, where it
was `create_gitlab_repo`.

**Tokens.** Two. `GITLAB_API_KEY` is the GitLab token, read when a tool is called, on both transports.
`MCP_AUTH_TOKEN` is the access token HTTP requires of a client (32 characters or more; HTTP exits 1 without it) and
is never read over stdio or sent to GitLab. `MCP_TRANSPORT`, `PORT` (default 3000) and `HOST` (default `0.0.0.0`) are the only other variables, and the
only flags are `--help` and `--version`.

**Verified.** The suite (302 tests) replaces `fetch` and reaches no host beyond loopback. Every request shape is
checked against GitLab's documentation and against mocks, and a read shape against public data on
`gitlab.com` where that was possible. The production-only dependency tree the image would install
serves every tool over a real stdio pipe, and the HTTP transport is tested over real
loopback sockets, including near-miss access tokens, the access token's absence from every request to GitLab, and real processes
that refuse to start open and that serve and stop on `SIGTERM`.

**Not verified.** **Nothing has been exercised with a GitLab token:** the environment the work was
done in had none. The write tools are the ones to prove first, and `wiki/guides/smoke-test.md` is the
walk and the list of behaviours the documentation leaves open. A job's log has never been read, since
GitLab gives it only to a token. The `Dockerfile` has never been built: there was no Docker daemon. The HTTP transport has not
been reached from another machine, behind a proxy, or with a real GitLab key.

**Not built.** No CI. No TLS, `Host` allow-list or rate limiting in the HTTP transport, and no identity per caller (decision
`two-token-auth.md`). No self-managed GitLab instances. Not published to npm (`private: true`). No git tag, no published image.

**Next.** The owner's smoke test, and whatever it finds. Anything it changes in a request is a patch
release with its own approval.
