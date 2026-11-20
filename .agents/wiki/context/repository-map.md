---
name: agent-wiki-context-repository-map
description: Orientation before touching this repository — what lives where, the commands, the entry point, and the gotchas that actually bite.
---

# Repository Map

Read this before changing anything here. Underlying facts live in `wiki/`; this page is the
agent-facing orientation and links out rather than restating.

## What this repository is

`LXAgents-MCP/gitlab` — an MCP server named `lxagents-gitlab` that gives an LLM access to GitLab
through the REST API, over stdio or HTTP. Rust, on the official MCP SDK (`rmcp`) and `axum`, built by `cargo` into one binary, `lxagents-gitlab`. It is a **consumer**
of the shared instruction set and the **producer** of the tool surface in `src/controllers/`; see
[`../../rules/repository.md`](../../rules/repository.md).

## What lives where

`src/` is `main.rs` (the binary) and `lib.rs` (the library the tests link) plus six folders by role — `routes/`, `controllers/` (the tools), `services/`, `models/`,
`utils/` and `config/`. Which module goes where, and why, is in
[`wiki/information/architecture.md`](../../../wiki/information/architecture.md#where-the-code-lives).

| Path | Contents | Touch it when |
|---|---|---|
| `src/main.rs` | The binary's one entry point: `--help`, `--version`, `MCP_TRANSPORT`, the stdio transport, `SIGINT` and `SIGTERM`. | The process contract. |
| `src/routes/http.rs` | The HTTP transport and the only listener: `POST /mcp`, `GET /healthz`, stateless, no session; `PORT` and `HOST` are read in `src/config/http_settings.rs`. | Anything about serving over HTTP. |
| `src/services/auth.rs` | The two secrets: the access token for HTTP (`MCP_AUTH_TOKEN`: the 32-character floor, the bearer parser, the constant-time comparison) and the GitLab key (`GITLAB_API_KEY`). | Anything about either secret. |
| `src/routes/server.rs` | `GitlabServer`: lists every tool, runs a call with panics caught and redacted, shapes results, and carries the `instructions` text every client reads at `initialize`. | The protocol surface. |
| `src/services/api.rs` | `gitlab()` and `branch_or_default`: the one place a GitLab URL is built and a call is made. | How a call is addressed. |
| `src/services/client.rs`, `src/services/failure.rs` | `call_api`, one request with a timeout and a size cap; and the wording of every failure. | How a call is made, or fails. |
| `src/models/address.rs`, `src/models/paging.rs` | `parse_repo`, `ref_name`, `ref_path`, `file_path`, `is_namespace_path`: path parts validated and encoded; `page_query` and `paging`. | A new kind of path part, or paging. |
| `src/services/context.rs` | `Ctx`: the environment, the GitLab backend (`reqwest` in the binary, a mock in the tests) and the clock, passed to every handler. | Anything a handler reads from outside. |
| `src/models/args.rs` | Argument checks: text, flags, whole numbers, choices, lists, label lists. | A new kind of argument. |
| `src/services/users.rs` | Username to user id, refusing an unknown name before any write. | A tool that names a person. |
| `src/utils/render.rs` | The one renderer from a payload to the text the model reads. | How a result reads. |
| `src/models/schemas.rs` | Shared JSON Schema fragments (`repo_arg`, paging) and the four annotation sets. | A fragment or hint several tools share. |
| `src/utils/text.rs` | `short`, `first_line`, `head`, `tail`, `clean_log`, cutting by character. | Cutting or shortening text. |
| `src/models/shapes.rs` | GitLab's objects as the tools report them — a project, a release, an issue or a job — each a schema and the function that fills it, and `expect_body`, which checks an answer is the object expected. | A second tool returns an object another already does. |
| `src/services/merge_status.rs` | What GitLab says about whether a merge request can be merged; it waits through the clock in `Ctx`, so a test can replace it. | Merging, or waiting on GitLab. |
| `src/config/config.rs` | Every constant: the variable name, site and API base and version pin, timeout, limits, the HTTP defaults. | Changing a limit. |
| `src/utils/errors.rs`, `src/utils/log.rs`, `src/config/version.rs` | `Refused`; the stderr logger; `VERSION` (from `Cargo.toml`) and `SERVER_NAME`. | Rarely. |
| `src/controllers/` | `mod.rs`, the registry and its `tools!` list, and one module per tool. | Adding or changing a tool — **a release**. |
| `Dockerfile`, `.dockerignore` | The image (stdio by default, HTTP when told to): `rust:1.97-alpine` builds `cargo build --release --locked`, `alpine:3.22` holds the binary and `ca-certificates`, `USER nobody`, `CMD ["lxagents-gitlab"]`, and nothing else. The build stage has been run; the whole image has not been built through. | The image changes — and say what was not built. |
| `test/` | One file per subject, all modules of `main.rs`, one test binary; plus `helpers.rs` and `surface.rs`, which every other file uses, and `fixtures/tools-list.json`, the pinned surface. | Always. Every behavioural change ships with a test. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. | You are changing something true only here. |
| `wiki/` | Human documentation, and `wiki/logs/` for release history. | A person needs to read it. |

There is no `src/transport/` and no `content/`; HTTP is `src/routes/http.rs` and nothing else. If a file you are about to
edit is under one of them, it does not exist here. `target/` is build output: never edit it, and never commit it.

## Entry point

There is **one**: `src/main.rs`, built into `target/release/lxagents-gitlab`, which is what an MCP client spawns. It reads `MCP_TRANSPORT`: stdio connects a
stdio transport and exits by itself when its client goes away, because stdin closes; `http` hands over to
`src/routes/http.rs`. Either handles `SIGINT` and `SIGTERM`, and nothing writes to stdout. Behind it,
`src/routes/server.rs` serves `GitlabServer`, which lists what the `tools!` list in `src/controllers/mod.rs` names.

## Commands

```bash
cargo build --release                            # target/release/lxagents-gitlab
cargo fmt --check                                # formatting
cargo clippy --all-targets -- -D warnings        # src/ and test/
cargo test                                       # the suite; the first run compiles every dependency
cargo run --release                              # stdio; GITLAB_API_KEY comes from the environment
MCP_TRANSPORT=http cargo run --release           # HTTP on :3000: with MCP_AUTH_TOKEN (32+ chars) on all interfaces, without it on 127.0.0.1 only
GITLAB_API_KEY=<token> npx @modelcontextprotocol/inspector target/release/lxagents-gitlab      # the MCP Inspector, to drive it by hand
```

## Gotchas that actually bite

* **Nothing writes to stdout.** stdout is the JSON-RPC channel. A stray write corrupts the stream
  and the client reports a JSON parse error that points at the payload, not at the logging.
* **Starting without the GitLab key is fine; HTTP without the access token is fine only on loopback.** The key is read when
  a tool runs; without it the tool refuses and names `GITLAB_API_KEY`, which is not a broken server. HTTP
  with no `MCP_AUTH_TOKEN` on an address that is not loopback, or a token under 32 characters anywhere, exits 1 on
  purpose. With neither `MCP_AUTH_TOKEN` nor `HOST` set it is a local install on `127.0.0.1`, and says so.
* **A server registered mid-session is absent until the session restarts.** The client loads its
  connectors at start.
* **The tool list is pinned.** Add a tool without its line in `test/surface.rs` and three tests
  fail. That is the design, not friction.
* **`MockApi` fails on any request it was not told about.** If a tool makes one more call than the
  test queued, the failure reads `unexpected request #n`, and it is correct.
* **A client runs the last build.** It runs `target/release/lxagents-gitlab`, so a change to `src/` is not in it
  until `cargo build --release`. The tests build their own debug binary and do not see `target/release/`.
* **GitLab answers 404 for a private project the token cannot see**, so a 404 can mean
  "not visible", not "does not exist". The refusal says so.
* **A tool file's name is its tool's name.** Renaming the file is renaming the tool, which is a
  breaking change for any client that calls it.

## Generated and vendored paths

`target/`, which `cargo` generates and `.gitignore` excludes. Nothing is vendored.

## Where the shared set resolves from

The connector `lxagents-shared-instruction`, never a file in this repository.

## Further reading

* [`../../../wiki/information/architecture.md`](../../../wiki/information/architecture.md) — how the server is built and why.
* [`../../../wiki/reference/mcp-surface.md`](../../../wiki/reference/mcp-surface.md) — every tool, and what the server does not expose.
* [`../../../wiki/environments/env.md`](../../../wiki/environments/env.md) — the one variable, and the token's required permissions.
* [`../security/security-boundaries.md`](../security/security-boundaries.md) — the security procedure.
