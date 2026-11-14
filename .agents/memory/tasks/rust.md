---
name: memory-tasks-rust
description: Rewriting the server and its tests from TypeScript to Rust with the official MCP SDK (rmcp) and axum, with the same 45 tools, tokens, variables and routes; done alongside github, cli and template.
---

# Rust rewrite — GitLab

## 2026-11-14 — planned

**Goal.** The owner asked for every repository to move from TypeScript to Rust, using the official Rust MCP
SDK (`rmcp`, <https://github.com/modelcontextprotocol/rust-sdk>) and `axum` as the web framework, each
repository in its own chain, planned in detail first. The owner is away and asked for the recommended option
wherever a question comes up, and allowed merging without review.

**Objective.**

* `src/` and `test/` are Rust; `package.json`, the lockfile, `tsconfig*.json` and every `.ts` file are gone.
* `cargo build --release` builds one binary, `lxagents-gitlab`, which is what a client runs.
* The 45 tools are unchanged: their names, descriptions, input and output schemas and annotations match the
  `tools/list` of 0.7.0, pinned in `test/fixtures/tools-list.json` and compared by a test.
* Every request a tool sends, every summary, field and refusal, the rendered text, both transports, both
  secrets, the variables, the flags and the exit codes are unchanged; every TypeScript test has a Rust
  counterpart and passes.
* `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

**Status:** planned. The full plan, with the file-for-file map, the design and the test map, is the project
report `reports/rust-gitlab-plan.md`.

**Detail.** A rewrite in another language, not a redesign: each file keeps its one job, in the same six
folders. The shared instruction connector was not available in the session that planned this, so
`commit_conventions` and the other shared tools could not be read; the chain follows this repository's own
history and the project memory instead. Baseline: 0.7.0, `npm test` 308 passing.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `plan/rust` | |
| 2 | Rewrite src/ and test/ in Rust | `refactor/rust` | |
| 3 | Corrected documentation | `docs/rust` | |
| 4 | Release | `release/0.8.0` | |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. The same chain runs in `github`, `cli`
and `template`; no chain depends on another.

## Decisions

| # | Decision |
|---|---|
| R1 | The release is `0.8.0`: no tool changes, but what a client runs moves from `node dist/index.js` to the `lxagents-gitlab` binary. No tag, nothing published. |
| R2 | One package at the root, a library and a binary, so tests reach the modules. `Cargo.lock` is committed. Edition 2024, `rust-version` 1.88 (rmcp's). |
| R3 | Tests stay in `test/`, as this repository places them, as one integration-test binary declared in `Cargo.toml` (`autotests = false`). |
| R4 | Everything a handler reads from outside — the environment, the network, the clock — comes through one injected context, because parallel Rust tests cannot share process environment or replace a global `fetch`. The GitLab key is still read per call. |
| R5 | Schemas are JSON built by hand and pinned to the 0.7.0 golden file, so the published surface is the same; no schema derive, whose output differs from zod's. |
| R6 | Strings are measured and cut in Unicode scalar values, never mid-character; JavaScript counted UTF-16 units, which differs only beyond the Basic Multilingual Plane. |
| R7 | The tool registry is a static list in `src/controllers/mod.rs`; a test holds files and tools together in both directions, as discovery did. |
| R8 | The runtime dependencies grow beyond the SDK, as Rust's standard library has no HTTP, TLS, async runtime or JSON; each is listed with its reason in `decisions/rust-runtime-dependencies.md`. |
| R9 | The rules and wiki pages that state the stack are corrected in task 3, as the owner's request is the agreement to change it; any new rule found on the way is proposed, not written. |

## Task entries

### Task 1 — plan/rust

Landed: this record, `decisions/rust-runtime-dependencies.md`, and their rows in `memory-index.md`. Nothing
outside `.agents/` changes. The `PR` column is filled by the release task.
