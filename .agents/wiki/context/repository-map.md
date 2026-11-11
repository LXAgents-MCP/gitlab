---
name: agent-wiki-context-repository-map
description: Orientation before touching this repository — what lives where, the commands, the entry point, and the gotchas that actually bite.
---

# Repository Map

Read this before changing anything here. Underlying facts live in `wiki/`; this page is the
agent-facing orientation and links out rather than restating.

## What this repository is

`LXAgents-MCP/gitlab` — an MCP server named `lxagents-gitlab` that gives an LLM access to GitLab
through the REST API, over stdio or HTTP. TypeScript, Node ESM, compiled to `dist/` by `tsc`. It is a **consumer**
of the shared instruction set and the **producer** of the tool surface in `src/controllers/`; see
[`../../rules/repository.md`](../../rules/repository.md).

## What lives where

`src/` is `index.ts` plus six folders by role — `routes/`, `controllers/` (the tools), `services/`, `models/`,
`utils/` and `config/`. Which module goes where, and why, is in
[`wiki/information/architecture.md`](../../../wiki/information/architecture.md#where-the-code-lives).

| Path | Contents | Touch it when |
|---|---|---|
| `src/index.ts` | The one entry point: `--help`, `--version`, `MCP_TRANSPORT`, the stdio transport, `SIGINT` and `SIGTERM`. | The process contract. |
| `src/routes/http.ts` | The HTTP transport and the only listener: `POST /mcp`, `GET /healthz`, a fresh stateless server per request; `PORT` and `HOST` are read in `src/config/http-settings.ts`. Also what `npm run start:http` runs. | Anything about serving over HTTP. |
| `src/services/auth.ts` | The two secrets: the access token for HTTP (`MCP_AUTH_TOKEN`: the 32-character floor, the bearer parser, the constant-time comparison) and the GitLab key (`GITLAB_API_KEY`). | Anything about either secret. |
| `src/routes/server.ts` | Builds one `McpServer`, registers every tool, shapes results, and carries the `instructions` text every client reads at `initialize`. | The protocol surface. |
| `src/services/api.ts` | `gitlab()` and `branchOrDefault`: the one place a GitLab URL is built and a call is made. | How a call is addressed. |
| `src/services/client.ts`, `src/services/failure.ts` | `callApi`, one request with a timeout and a size cap; and the wording of every failure. | How a call is made, or fails. |
| `src/models/address.ts`, `src/models/paging.ts` | `parseRepo`, `refName`, `refPath`, `filePath`, `isNamespacePath`: path parts validated and encoded; `pageQuery` and `paging`. | A new kind of path part, or paging. |
| `src/models/api-body.ts` | `ApiBody`, the type of an answer from GitLab: external JSON, the one loose type. | Never, without a reason to type answers per endpoint. |
| `src/models/args.ts` | Argument checks: text, flags, whole numbers, choices, lists, label lists. | A new kind of argument. |
| `src/services/users.ts` | Username to user id, refusing an unknown name before any write. | A tool that names a person. |
| `src/utils/render.ts` | The one renderer from a payload to the text the model reads. | How a result reads. |
| `src/models/schemas.ts` | Shared zod fragments (`repoArg`, paging) and the four annotation sets. | A fragment or hint several tools share. |
| `src/utils/text.ts` | `short`, `firstLine`, `head`, `tail`, `cleanLog`. | Cutting or shortening text. |
| `src/models/shapes.ts` | GitLab's objects as the tools report them — a project, a release, an issue or a job — each a schema and the function that fills it, and `expectBody`, which checks an answer is the object expected. | A second tool returns an object another already does. |
| `src/services/merge-status.ts`, `src/utils/wait.ts` | What GitLab says about whether a merge request can be merged; the one `sleep`, so a test can replace the clock. | Merging, or waiting on GitLab. |
| `src/config/config.ts` | Every constant: the variable name, site and API base and version pin, timeout, limits, the HTTP defaults. | Changing a limit. |
| `src/utils/errors.ts`, `src/utils/log.ts`, `src/config/version.ts` | `Refused`; the stderr logger; `ROOT`, `VERSION` and `SERVER_NAME`. | Rarely. |
| `src/controllers/` | `index.ts`, the registry, and one module per tool. | Adding or changing a tool — **a release**. |
| `Dockerfile`, `.dockerignore` | The image (stdio by default, HTTP when told to): `node:22-alpine` in two stages, a build stage that runs `npm ci --ignore-scripts` and `npm run build`, then `npm ci --ignore-scripts --omit=dev` and `dist/` copied in, `USER node`, `CMD ["node", "dist/index.js"]`, and nothing else. It has never been built. | The image changes — and say it is unbuilt if it still is. |
| `test/` | One file per subject, plus `helpers.ts` and `surface.ts`, which every other file imports. | Always. Every behavioural change ships with a test. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. | You are changing something true only here. |
| `wiki/` | Human documentation, and `wiki/logs/` for release history. | A person needs to read it. |

There is no `src/transport/` and no `content/`; HTTP is `src/routes/http.ts` and nothing else. If a file you are about to
edit is under one of them, it does not exist here. `dist/` is build output: never edit it, and never commit it.

## Entry point

There is **one**: `src/index.ts`, compiled to `dist/index.js`, which is what an MCP client spawns. It reads `MCP_TRANSPORT`: stdio connects a
stdio transport and exits by itself when its client goes away, because stdin closes; `http` hands over to
`src/routes/http.ts`. Either handles `SIGINT` and `SIGTERM`, and nothing writes to stdout. Behind it,
`src/routes/server.ts` calls `createServer()`, which registers whatever `src/controllers/index.ts` discovered. (`npm run
start:http` runs `dist/routes/http.js` directly.)

## Commands

```bash
npm install
npm run build                               # src/ to dist/, which start, start:http and inspect run
npm run typecheck                           # src/ and test/
npm test                                    # the sources through tsx; no build needed
npm start                                   # stdio; GITLAB_API_KEY comes from the environment
npm run start:http                          # HTTP on :3000: with MCP_AUTH_TOKEN (32+ chars) on all interfaces, without it on 127.0.0.1 only
GITLAB_API_KEY=<token> npm run inspect      # the MCP Inspector, to drive it by hand
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
* **The tool list is pinned.** Add a tool without its line in `test/surface.ts` and three tests
  fail. That is the design, not friction.
* **`mockApi` fails on any request it was not told about.** If a tool makes one more call than the
  test queued, the failure reads `unexpected request #n`, and it is correct.
* **`npm start` runs the last build.** It runs `dist/`, so a change to `src/` is not in it until `npm run build`.
  The tests run the sources and do not see `dist/` at all.
* **GitLab answers 404 for a private project the token cannot see**, so a 404 can mean
  "not visible", not "does not exist". The refusal says so.
* **A tool file's name is its tool's name.** Renaming the file is renaming the tool, which is a
  breaking change for any client that calls it.

## Generated and vendored paths

`node_modules/`, and `dist/`, which `npm run build` generates and `.gitignore` excludes.

## Where the shared set resolves from

The connector `lxagents-shared-instruction`, never a file in this repository.

## Further reading

* [`../../../wiki/information/architecture.md`](../../../wiki/information/architecture.md) — how the server is built and why.
* [`../../../wiki/reference/mcp-surface.md`](../../../wiki/reference/mcp-surface.md) — every tool, and what the server does not expose.
* [`../../../wiki/environments/env.md`](../../../wiki/environments/env.md) — the one variable, and the token's required permissions.
* [`../security/security-boundaries.md`](../security/security-boundaries.md) — the security procedure.
