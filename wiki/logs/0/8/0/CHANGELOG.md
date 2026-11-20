# 0.8.0

**Released:** 2026-11-20

**The server is Rust.** `src/` and `test/` are Rust, on the official MCP SDK (`rmcp` 3.10) and `axum` 0.8, and
`cargo build --release` builds one binary, `target/release/lxagents-gitlab`, which is what runs. The 45 tools,
their arguments, results and messages, the tokens, the variables, the routes, the flags, the exit codes and the
local-install rules are all as they were; `tools/list` is compared against 0.7.0's by a test. What moves is what
a client runs: `node dist/index.js` is now the binary.

**Clients must:** install Rust 1.88 or newer, run `cargo build --release` in the checkout, change the client's
`command` from `node` (with `args` `dist/index.js`) to `/absolute/path/to/gitlab/target/release/lxagents-gitlab`
with no `args`, and restart the client. `MCP_TRANSPORT=http` replaces running `dist/routes/http.js` directly. A
rebuilt image already runs the binary.

## Added

* `Cargo.toml` and `Cargo.lock`: one package, a library and the `lxagents-gitlab` binary, edition 2024.
* Runtime dependencies `rmcp`, `axum`, `tokio`, `reqwest` (rustls), `serde`, `serde_json`, `sha2`, `subtle`,
  `base64`, `percent-encoding`, `regex`, `chrono`, `encoding_rs`, `futures`, `tower`, `http-body-util` and
  `bytes`, each with its reason in `.agents/memory/decisions/rust-runtime-dependencies.md`.
* `test/fixtures/tools-list.json`, 0.7.0's `tools/list`, which the registry test compares the surface against.

## Changed

* Every `.ts` file in `src/` and `test/` is a `.rs` file with the same job, in the same folders.
* Every handler reads the environment, reaches GitLab and waits through one injected context, so the tests
  replace them without touching the process.
* `--help` says `Usage: lxagents-gitlab [options]`.
* Text is measured and cut by character, not by UTF-16 unit; the two differ only for characters beyond the
  Basic Multilingual Plane.
* The `Dockerfile` builds with `rust:1.97-alpine` into a static binary and runs it on `alpine:3.22` as
  `nobody`, with `CMD ["lxagents-gitlab"]`.

## Fixed

* A `SIGINT` or `SIGTERM` sent the moment the startup line appeared could end the process with the default
  action; the handlers are now installed before the line is written, on both transports.
* A request header with a byte outside ASCII is read as Latin-1, as Node read it, so such a credential
  is a wrong access token (`invalid_token`) rather than a missing one.

## Removed

* `package.json`, `package-lock.json`, `tsconfig.json`, `tsconfig.test.json`, and the npm scripts.

## Verified

`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with no output. `cargo test`: 306
passing, none failing — 0.7.0's 308 cases, less two that checked a function reading the environment when
given no value, which no Rust function does. The release binary prints `lxagents-gitlab 0.8.0`. The image's
build stage ran in a container and its binary ran; the runtime stage was not built, and still nothing has run
against live GitLab.
