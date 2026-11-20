---
name: tool-surface
description: Adding, changing or removing a tool changes what every client sees — the file contract, annotations, the pinned list, and the release obligation.
---

# Tool Surface

`src/controllers/` is not a source folder. It is the **product**: every module in it is a tool a client
can call, and a client's configuration, or a person's habits, may name any of them.

A module added there is served on the next boot. There is no draft space and no staging area
inside it.

## The boundary

| Change to… | Effect |
|---|---|
| `src/controllers/**` | Changes the surface. A new tool is a **minor** release; a rename, a removal or a changed argument is a **major** one. |
| `src/services/api.rs`, `src/services/client.rs`, `src/services/failure.rs`, `src/models/address.rs`, `src/utils/render.rs`, `src/routes/server.rs`, `src/models/args.rs` | Changes how every tool behaves. A release only when a client can see the difference. |
| `.agents/**`, `wiki/**`, `test/**` | Local. Published to nobody. |

## The file contract

* The file name is the tool name: `repo_create.rs` is `repo_create`. A module is served only once its
  name is in the `tools!` list in `src/controllers/mod.rs`, in file-name order. The registry test holds
  files, that list and tools together in all directions, so a stray file, an unlisted module, or a tool
  in the wrong file, fails.
* A module has `pub fn def() -> ToolDef` and `pub async fn handle(ctx: &Ctx, args: &Args) ->
  Result<Outcome, Refused>`. Helpers belong in `src/`, not here.
* `ToolDef` has `name`, `description`, `input_schema`, `output_schema` and `annotations`, all required.
  The registry test fails on a tool without a useful description, an output schema, or the four
  hints.
* `handle` returns an `Outcome`: a one-sentence `summary`, the fields, and `ok`. The fields satisfy the
  `output_schema`, which `expect_output` in `test/helpers.rs` checks. A failure is an `Err(Refused)`.
* Two modules with one tool name fail at boot, not at first call.

## Arguments and results

* `repo` is the first argument of every tool that acts on a repository, which GitLab calls a
  project: its full path, `group/subgroup/project`, or its numeric id, parsed by `parse_repo`. Only
  `repo_create` and `repo_search` take none.
* Validate arguments before the token is read.
* Build every URL through `src/services/api.rs`. A path segment that did not come from `parse_repo`,
  `ref_name`/`ref_path` or `file_path` has not been validated.
* A list takes `page` and `per_page` and returns `page`, `per_page` and `has_more`.
* Long text — a file, a log, a diff — goes in `text`, cut to a stated limit with `truncated: true`
  when it was cut. The renderer fences it.
* A description says what the tool does and when to reach for it, in about 250 characters or fewer:
  the whole set sits in the client's context at once.

## The pinned list

`test/surface.rs` writes out every tool and tags it `read`, `write`, `update` or `destroy`. The
registry, protocol and stdio tests all compare against it, and the registry test holds each tool's
annotations to its kind. Adding a tool is one line there; a tool that appears or vanishes without
one fails three tests.

## Annotations are claims

| Kind | Hints |
|---|---|
| `read` | read-only, idempotent |
| `write` | creates something; repeating the call repeats the effect |
| `update` | changes something that exists; repeating it changes nothing more |
| `destroy` | deletes, overwrites or merges |

All four carry `openWorldHint`. A client decides whether to ask the owner first from these hints,
so a hint that is wrong is worse than none.

## Destructive tools

`repo_delete` refuses unless `confirm` equals the project's full path. That is a mechanical
gate, and a test asserts the refusal. Deleting a branch, tag, release or file, and merging, are
annotated `destroy` and rely on the client asking; their descriptions say to confirm with the
owner. That second kind is text, not mechanism: nothing in this repository can make a client ask.

## A tool change is a release

Clients pick a change up the next time they start the server, so the release log is the only
notice they get.

1. Decide the version with the owner first — the shared `versioning` tool. Never bump unasked.
2. Add `wiki/logs/{Major}/{Minor}/{Patch}/CHANGELOG.md`, with the **Clients must** line filled in:
   nothing, rename a tool they call, or move a token.
3. Add its row to `.agents/index/logs-index.md`, newest first.
4. Update the version in `Cargo.toml` and the package's own entry in `Cargo.lock`.
5. Add or change the tool's row in `wiki/reference/mcp-surface.md` **in the same commit as the
   tool**.

## Restart to see a change

The registry is compiled in and read once, at boot. Editing `src/controllers/` has no effect until the binary is rebuilt and the
process restarted: rebuild, then restart the client or the server. `npx @modelcontextprotocol/inspector target/release/lxagents-gitlab` shows the surface a fresh process would build.
