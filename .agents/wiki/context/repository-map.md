---
name: agent-wiki-context-repository-map
description: Orientation before touching this repository — what lives where, the commands, the entry point, and the gotchas that actually bite.
---

# Repository Map

Read this before changing anything here. Underlying facts live in `wiki/`; this page is the
agent-facing orientation and links out rather than restating.

## What this repository is

`LXAgents-MCP/gitlab` — an MCP server named `lxagents-gitlab` that gives an LLM access to GitLab
through the REST API, over stdio. Plain JavaScript, Node ESM, no build step. It is a **consumer**
of the shared instruction set and the **producer** of the tool surface in `src/tools/`; see
[`../../rules/repository.md`](../../rules/repository.md).

## What lives where

| Path | Contents | Touch it when |
|---|---|---|
| `src/index.js` | The one entry point: `--help`, `--version`, the stdio transport, `SIGINT`. | The process contract. |
| `src/server.js` | Builds one `McpServer`, registers every tool, shapes results, and carries the `instructions` text every client reads at `initialize`. | The protocol surface. |
| `src/api.js` | The GitLab client: the token, addressing (`parseRepo`, `refName`, `refPath`, `filePath`), `callApi`, `gitlab()`, the wording of every failure, paging. | How a call is addressed, made, or fails. |
| `src/args.js` | Argument checks: text, flags, whole numbers, choices, lists. | A new kind of argument. |
| `src/render.js` | The one renderer from a payload to the text the model reads. | How a result reads. |
| `src/schemas.js` | Shared zod fragments (`repoArg`, paging) and the four annotation sets. | A fragment or hint several tools share. |
| `src/text.js` | `short`, `firstLine`, `head`, `tail`. | Cutting or shortening text. |
| `src/config.js` | Every constant: the variable name, API base and version pin, timeout, limits. | Changing a limit. |
| `src/errors.js`, `src/log.js`, `src/version.js` | `Refused`; the stderr logger; `ROOT`, `VERSION` and `SERVER_NAME`. | Rarely. |
| `src/tools/` | `index.js`, the registry, and one module per tool. | Adding or changing a tool — **a release**. |
| `test/` | One file per subject, plus `helpers.js` and `surface.js`, which every other file imports. | Always. Every behavioural change ships with a test. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. | You are changing something true only here. |
| `wiki/` | Human documentation, and `wiki/logs/` for release history. | A person needs to read it. |

There is no `dist/`, no `src/transport/`, no `content/` and no HTTP code. If a file you are about to
edit is under one of those paths, it does not exist here.

## Entry point

There is **one**: `src/index.js`, what an MCP client spawns. It connects a stdio transport, handles
`SIGINT`, and writes nothing to stdout. The process exits by itself when its client goes away,
because stdin closes. Behind it, `src/server.js` calls `createServer()`, which registers whatever
`src/tools/index.js` discovered.

## Commands

```bash
npm install
npm test
npm start                                   # stdio; the token comes from the environment
LXAGENTS_MCP_GITLAB_API_KEY=<token> npm run inspect   # the MCP Inspector, to drive it by hand
```

## Gotchas that actually bite

* **Nothing writes to stdout.** stdout is the JSON-RPC channel. A stray write corrupts the stream
  and the client reports a JSON parse error that points at the payload, not at the logging.
* **Starting without a token is fine.** The token is read when a tool runs; without it the tool
  refuses and names `LXAGENTS_MCP_GITLAB_API_KEY`. That is not a broken server.
* **A server registered mid-session is absent until the session restarts.** The client loads its
  connectors at start.
* **The tool list is pinned.** Add a tool without its line in `test/surface.js` and three tests
  fail. That is the design, not friction.
* **`mockApi` fails on any request it was not told about.** If a tool makes one more call than the
  test queued, the failure reads `unexpected request #n`, and it is correct.
* **`node --test` runs `helpers.js` and `surface.js` too.** They define no tests; the runner's total
  counts them.
* **GitLab answers 404 for a private project the token cannot see**, so a 404 can mean
  "not visible", not "does not exist". The refusal says so.
* **A tool file's name is its tool's name.** Renaming the file is renaming the tool, which is a
  breaking change for any client that calls it.

## Generated and vendored paths

`node_modules/` only. Nothing is generated into the tree.

## Where the shared set resolves from

The connector `lxagents-shared-instruction`, never a file in this repository.

## Further reading

* [`../../../wiki/information/architecture.md`](../../../wiki/information/architecture.md) — how the server is built and why.
* [`../../../wiki/reference/mcp-surface.md`](../../../wiki/reference/mcp-surface.md) — every tool, and what the server does not expose.
* [`../../../wiki/environments/env.md`](../../../wiki/environments/env.md) — the one variable, and the token's required permissions.
* [`../security/security-boundaries.md`](../security/security-boundaries.md) — the security procedure.
