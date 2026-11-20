---
name: gitlab-repository-rules
description: Rules specific to LXAgents-MCP/gitlab — its two roles, the stack and commands, the code conventions, and what must not be introduced.
---

# GitLab Repository Rules

Rules that are true of **this repository only**. Anything true of more than one repository is
proposed to the owner for the shared set, not written here.

This file is a hub. It links out rather than restating, and it never repeats a rule the shared
set already carries.

## This repository has two roles

| Role | Meaning |
|---|---|
| **Consumer** | It follows the shared instruction set, read through the connector, plus the local additions in `.agents/`. |
| **Producer** | It serves the GitLab tool surface: one module per tool in `src/controllers/`. |

Keep them apart. `.agents/` is this repository's own instruction set; `src/controllers/` is the product.
Nothing from the shared set is copied here.

## What lives where

| Path | Holds |
|---|---|
| `src/` | The server: `main.rs` (the binary), `lib.rs` (the library the tests link) and one folder per role — `routes/`, `controllers/`, `services/`, `models/`, `utils/`, `config/` (see [Where the code lives](../../wiki/information/architecture.md#where-the-code-lives)). `src/controllers/` is the tool surface; everything else in `src/` is shared by it. |
| `test/` | The server's tests. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. |
| `wiki/` | Human documentation, and the release logs under `wiki/logs/`. |
| `Dockerfile`, `.dockerignore` | The image: stdio by default, HTTP with `MCP_TRANSPORT=http`. See [`wiki/environments/docker.md`](../../wiki/environments/docker.md). |

Full orientation: [`../wiki/context/repository-map.md`](../wiki/context/repository-map.md).

## Stack and commands

Rust, edition 2024, `rust-version` 1.88, on the official MCP SDK (`rmcp`) and `axum`. **The build is `cargo`
alone**: one package at the root, a library and the `lxagents-gitlab` binary, with `Cargo.lock` committed — do
not add a build script, a workspace or a second build tool without agreement.

| Task | Command |
|---|---|
| Build | `cargo build --release` — `target/release/lxagents-gitlab` |
| Format | `cargo fmt` (`cargo fmt --check` before a commit) |
| Lint `src/` and `test/` | `cargo clippy --all-targets -- -D warnings` |
| Test | `cargo test` |
| Run (stdio) | `cargo run --release` |
| Run (HTTP) | `MCP_TRANSPORT=http cargo run --release` |
| Inspect the MCP surface | `npx @modelcontextprotocol/inspector target/release/lxagents-gitlab` |
| Build the image | `docker build -t "lxagents-gitlab:$VERSION" .` — it has never been built here |

**The first `cargo test` in a checkout downloads and compiles every dependency**, which takes minutes and
needs the network; later runs are quick. A test that waits on the binary (`test/stdio.rs`, `test/http.rs`)
runs the debug build Cargo makes for the suite, not `target/release/`.

## Code conventions the codebase already follows

* **No `unsafe`, and no `unwrap` on anything from outside.** A GitLab answer is a `serde_json::Value`, the
  one loose type, and a tool checks the field it relies on before reading it (`src/models/shapes.rs`).
* **Everything from outside comes through `Ctx`** (`src/services/context.rs`): the environment, the GitLab
  backend and the clock. A handler never reads `std::env` or opens a connection itself.
* **Nothing writes to stdout.** On stdio, stdout *is* the JSON-RPC channel. Diagnostics go through
  `src/utils/log.rs`, which writes to stderr, on the HTTP transport too. There is no `print!` or `println!` anywhere
  in `src/`.
* **One `GitlabServer` per connection.** It holds no state between calls; over HTTP the service runs
  stateless, with no session.
* **Every URL is built in `src/services/api.rs`.** A project, a reference and a file path are validated
  and percent-encoded in `src/models/address.rs`, and a tool never concatenates its own.
* **Arguments are checked before the token is read**, so a bad call costs no request and the
  refusal names the argument.
* **The GitLab key is read per call**, through `src/services/auth.rs` and nowhere else, from `GITLAB_API_KEY`, on
  both transports. It is never an argument, and is scrubbed from every message — refusals and crashes
  alike. **The access token is a different secret**: `MCP_AUTH_TOKEN` guards the HTTP door of a server
  install, is compared and dropped, and is never used against GitLab or read over stdio. A local install,
  on loopback with no token, is guarded by the `Host` and `Origin` checks in `src/routes/http.rs` instead.
* **A failure is an `Err(Refused)`** that says what to do next. A tool never returns an error
  payload, and a panic is caught, redacted and reported as an internal error, never as its explanation.
* **Comments explain why, not what.**

## Testing

Every behavioural change ships with a test in `test/`, one integration binary (`test/main.rs`, declared in
`Cargo.toml` as `suite`). The suite must pass before any commit, and **no test reaches GitLab or any host beyond this
machine**: the GitLab backend in `Ctx` is the `MockApi` of `test/helpers.rs`, which fails the test on any request
it was not told to expect. The exceptions are the redirect and size tests in `test/api.rs`, which use the
real client against servers on loopback, to show that the token is not sent to a second host.

Each tool is tested for: a success that asserts the exact request — method, URL, query, body — and
checks the payload against the tool's own output schema with `expect_output`; at least one failure
status; and argument validation that refuses before any request. `test/surface.rs` is the one
place the tool list is pinned. Prefer a test that pins an invariant over one that pins a string.

`cargo test` runs `test/main.rs`, which names every test file as a module; the two support files,
`helpers.rs` and `surface.rs`, hold no tests.

## What must not be introduced

* A build step beyond `cargo`, or any build output committed to the repository: `target/` is ignored.
* A dependency added to serve one call site. The runtime dependencies are listed with their reasons in
  [`../memory/decisions/rust-runtime-dependencies.md`](../memory/decisions/rust-runtime-dependencies.md); another
  needs its own record under `.agents/memory/decisions/` before it lands, not after.
* A transport other than stdio and the HTTP one in `src/routes/http.rs`, a listener anywhere else, a route
  beyond `POST /mcp` and `GET /healthz`, a way for an HTTP request to get through without the access
  token when one is set, HTTP starting without one on an address that is not loopback, a local install
  without the `Host` and `Origin` checks, a client's bearer used as the GitLab key (or the other way round),
  or a store of tokens between requests. See
  [`../memory/decisions/two-token-auth.md`](../memory/decisions/two-token-auth.md). The
  image follows: no `EXPOSE`, no `ENTRYPOINT`, no `ENV` line holding a secret, no compose file.
* An environment variable, flag or configuration file without the owner's say-so. The server reads
  `GITLAB_API_KEY` and `MCP_AUTH_TOKEN` (the two secrets) and `MCP_TRANSPORT`, `PORT` and `HOST`, and takes
  `--help` and `--version`; another of either is a change to a public surface, not an implementation
  detail.
* A tool argument that carries a credential, or any way for a caller to choose the host. The API
  base is a constant in `src/config/config.rs`, which is also why self-managed GitLab instances are not supported.
* A URL built anywhere but `src/services/api.rs`.
* A third documentation tree. `wiki/` and `.agents/wiki/` are the only two.
* `content/`, or any copy of the shared set. This repository reads it through the connector.

## Deployment

Not published: `Cargo.toml` sets `publish = false`, and the client runs the server by path
(`target/release/lxagents-gitlab`, after `cargo build --release`) with the key in the `env` block of its own configuration, or runs it as an
HTTP service the client reaches with an access token. Making it
installable from crates.io is a one-line change the owner approves, and not one to make in passing. A
`Dockerfile` builds an image that speaks stdio for a client that runs `docker run -i`, or HTTP when
told to; publishing it to a registry is the owner's decision as well.
