---
name: tool-surface
description: Adding, changing or removing a tool changes what every client sees — the file contract, annotations, the pinned list, and the release obligation.
---

# Tool Surface

`src/tools/` is not a source folder. It is the **product**: every module in it is a tool a client
can call, and a client's configuration, or a person's habits, may name any of them.

A module added there is served on the next boot. There is no draft space and no staging area
inside it.

## The boundary

| Change to… | Effect |
|---|---|
| `src/tools/**` | Changes the surface. A new tool is a **minor** release; a rename, a removal or a changed argument is a **major** one. |
| `src/api.js`, `src/render.js`, `src/server.js`, `src/args.js` | Changes how every tool behaves. A release only when a client can see the difference. |
| `.agents/**`, `wiki/**`, `test/**` | Local. Published to nobody. |

## The file contract

* The file name is the tool name in kebab case: `repo-create.js` is `repo_create`. The registry
  test holds files and tools together in both directions, so a stray file, or a tool in the wrong
  file, fails.
* A module exports `TOOL` and `handle(args)`. A module that exports neither is skipped, so a helper
  dropped into `src/tools/` is never callable — but helpers belong in `src/`, not here.
* `TOOL` has `name`, `description`, `inputSchema`, `outputSchema` and `annotations`, all required.
  The registry test fails on a tool without a useful description, an output schema, or the four
  hints.
* `handle` returns `{ payload, ok }`. The payload is a one-sentence `summary` plus fields that
  satisfy the `outputSchema`, which `expectOutput` in `test/helpers.js` checks. A failure is a
  thrown `Refused`.
* Two modules with one tool name fail at boot, not at first call.

## Arguments and results

* `repo` is the first argument of every tool that acts on a repository, which GitLab calls a
  project: its full path, `group/subgroup/project`, or its numeric id, parsed by `parseRepo`. Only
  `repo_create` and `repo_search` take none.
* Validate arguments before the token is read.
* Build every URL through `src/api.js`. A path segment that did not come from `parseRepo`,
  `refName`/`refPath` or `filePath` has not been validated.
* A list takes `page` and `per_page` and returns `page`, `per_page` and `has_more`.
* Long text — a file, a log, a diff — goes in `text`, cut to a stated limit with `truncated: true`
  when it was cut. The renderer fences it.
* A description says what the tool does and when to reach for it, in about 250 characters or fewer:
  the whole set sits in the client's context at once.

## The pinned list

`test/surface.js` writes out every tool and tags it `read`, `write`, `update` or `destroy`. The
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
4. Update `package.json` and the lockfile's own version.
5. Add or change the tool's row in `wiki/reference/mcp-surface.md` **in the same commit as the
   tool**.

## Restart to see a change

The registry is read once, at boot. Editing `src/tools/` has no effect on a running process:
restart the client or the server. `npm run inspect` shows the surface a fresh process would build.
