---
name: memory-state-repository-state
description: Current known state of LXAgents-MCP/gitlab — what exists, what is verified, and what is not built yet.
---

# Repository State

## 2026-11-20

**Version `0.8.0`.** `Cargo.toml` and the package's entry in `Cargo.lock` carry it, and the release log is
`wiki/logs/0/8/0/CHANGELOG.md`; earlier releases are `0/7/0`, `0/6/1`, `0/6/0`, `0/5/0`, `0/4/0`, `0/3/0`, `0/2/0` and `0/1/0`. 0.8.0 rewrites the server in Rust, one binary; 0.7.0 moved it to TypeScript compiled to `dist/`; 0.6.0 lets a
local install of the HTTP transport, on loopback, run without `MCP_AUTH_TOKEN`; a server install still requires it. No git tag was made, by decision (D3 in `tasks/forge-servers.md`, D8 in `tasks/http-token-auth.md`, T7 in
`tasks/two-token-auth.md`, H3 in `tasks/http-defaults.md`, A3 in `tasks/src-architecture.md`, L6 in `tasks/local-auth-token.md`, D4 in `tasks/code-structure.md`, T7 in `tasks/typescript.md`, R1 in `tasks/rust.md`).

**What it is.** An MCP server, stdio by default and HTTP behind an access token, `lxagents-gitlab`, over the GitLab REST API at `gitlab.com`.
Rust, edition 2024, on the official MCP SDK (`rmcp`) and `axum`, built by `cargo` into `target/release/lxagents-gitlab` (ignored, never committed); the tests are one integration binary in `test/`. The runtime dependencies are listed in `decisions/rust-runtime-dependencies.md`.

**Surface.** One tool per module in `src/controllers/`. The full list is pinned in `test/surface.rs` and in `test/fixtures/tools-list.json` and
described in `wiki/reference/mcp-surface.md`: projects, branches, files and directories, commits,
tags and releases, issues, merge requests and pipelines. A tool count is deliberately not stated
here: it goes stale at the next module. `repo_create` was moved here from the `cli` server, where it
was `create_gitlab_repo`.

**Tokens.** Two. `GITLAB_API_KEY` is the GitLab token, read when a tool is called, on both transports.
`MCP_AUTH_TOKEN` is the access token a server install requires of a client (32 characters or more; HTTP on an address
that is not loopback exits 1 without it) and is never read over stdio or sent to GitLab. A local install, on loopback
with no token, answers only requests whose `Host` and `Origin` name loopback (`decisions/local-install-without-token.md`).
`MCP_TRANSPORT`, `PORT` (default 3000) and `HOST` (default `0.0.0.0` with a token, `127.0.0.1` without) are the only other variables, and the
only flags are `--help` and `--version`.

**Verified.** The suite (306 tests, with `cargo fmt --check` and `cargo clippy -D warnings` clean) replaces the GitLab backend and reaches no host beyond loopback. Every request shape is
checked against GitLab's documentation and against mocks, and a read shape against public data on
`gitlab.com` where that was possible. The release binary serves every tool over a real stdio pipe, and the HTTP transport is tested over real
loopback sockets, including near-miss access tokens, the access token's absence from every request to GitLab, and real processes
that refuse to start open and that serve and stop on `SIGTERM`.

**Not verified.** **Nothing has been exercised with a GitLab token:** the environment the work was
done in had none. The write tools are the ones to prove first, and `wiki/guides/smoke-test.md` is the
walk and the list of behaviours the documentation leaves open. A job's log has never been read, since
GitLab gives it only to a token. The `Dockerfile`'s build stage ran in a container; its runtime stage has not been built, as Alpine's mirror was unreachable from there. The HTTP transport has not
been reached from another machine, behind a proxy, or with a real GitLab key.

**Not built.** No CI. No TLS, `Host` allow-list or rate limiting in the HTTP transport, and no identity per caller (decision
`two-token-auth.md`). No self-managed GitLab instances. Not published to crates.io (`publish = false`). No git tag, no published image.

**Next.** The owner's smoke test, and whatever it finds. Anything it changes in a request is a patch
release with its own approval.
