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
| **Producer** | It serves the GitLab tool surface: one module per tool in `src/tools/`. |

Keep them apart. `.agents/` is this repository's own instruction set; `src/tools/` is the product.
Nothing from the shared set is copied here.

## What lives where

| Path | Holds |
|---|---|
| `src/` | The server. `src/tools/` is the tool surface; everything else in `src/` is shared by it. |
| `test/` | The server's tests. |
| `.agents/` | This repository's own rules, indexes, agent wiki and memory. |
| `wiki/` | Human documentation, and the release logs under `wiki/logs/`. |
| `Dockerfile`, `.dockerignore` | The stdio image. See [`wiki/environments/docker.md`](../../wiki/environments/docker.md). |

Full orientation: [`../wiki/context/repository-map.md`](../wiki/context/repository-map.md).

## Stack and commands

Plain JavaScript, Node ESM, Node >= 20. **There is no build step** — do not add one, and do not
introduce TypeScript, a bundler, or a transpiler without agreement.

| Task | Command |
|---|---|
| Install | `npm install` |
| Test | `npm test` |
| Run (stdio) | `npm start` |
| Inspect the MCP surface | `npm run inspect` |
| Build the image | `docker build -t "lxagents-gitlab:$VERSION" .` — it has never been built here |

**`npm install` before `npm test`, once per checkout.** A fresh clone has no `node_modules`, and
the suite does not say so: the test files fail with `ERR_MODULE_NOT_FOUND`, which reads as broken
code rather than an uninstalled tree. Install first, then trust the number.

## Code conventions the codebase already follows

* **ESM only.** `import`/`export`, `.js` extensions in relative specifiers, no `require`.
* **Nothing writes to stdout.** On stdio, stdout *is* the JSON-RPC channel. Diagnostics go through
  `src/log.js`, which writes to stderr. There is no `console.log` anywhere in `src/`.
* **One `McpServer` per connection.** `createServer()` builds a fresh instance; never hoist one to
  module scope.
* **Every URL is built in `src/api.js`.** A project, a reference and a file path are validated
  and percent-encoded there, and a tool never concatenates its own.
* **Arguments are checked before the token is read**, so a bad call costs no request and the
  refusal names the argument.
* **The token is read per call** from `LXAGENTS_MCP_GITLAB_API_KEY`, is never an argument, and is
  scrubbed from every message — refusals and crashes alike.
* **A failure is a thrown `Refused`** that says what to do next. A tool never returns an error
  payload, and never lets a bare exception reach the client as its explanation.
* **Comments explain why, not what.**

## Testing

Every behavioural change ships with a test in `test/`, using `node:test` and
`node:assert/strict`. The suite must pass before any commit, and **no test touches the network**:
`fetch` is replaced by `mockApi` in `test/helpers.js`, which fails the test on any request it was
not told to expect.

Each tool is tested for: a success that asserts the exact request — method, URL, query, body — and
checks the payload against the tool's own output schema with `expectOutput`; at least one failure
status; and argument validation that refuses before any request. `test/surface.js` is the one
place the tool list is pinned. Prefer a test that pins an invariant over one that pins a string.

`node --test` also executes the two support files under `test/` (`helpers.js`, `surface.js`),
which define no tests, so the runner's total includes them.

## What must not be introduced

* A build step, or any compiled output committed to the repository.
* A dependency added to serve one call site. The runtime dependencies are the MCP SDK and zod; a
  third needs its own record under `.agents/memory/decisions/` before it lands, not after.
* A transport other than stdio, a listener, or a route. See
  [`../memory/decisions/stdio-only-transport.md`](../memory/decisions/stdio-only-transport.md).
  The image follows: no `EXPOSE`, no `ENTRYPOINT`, no `ENV` line holding a token, no compose file.
* An environment variable, flag or configuration file without the owner's say-so. The server reads
  one variable and takes `--help` and `--version`; a second of either is a change to a public
  surface, not an implementation detail.
* A tool argument that carries a credential, or any way for a caller to choose the host. The API
  base is a constant in `src/config.js`, which is also why self-managed GitLab instances are not supported.
* A URL built anywhere but `src/api.js`.
* A third documentation tree. `wiki/` and `.agents/wiki/` are the only two.
* `content/`, or any copy of the shared set. This repository reads it through the connector.

## Deployment

Not published: `package.json` is `private`, and the client runs the server by path
(`node src/index.js`) with the token in the `env` block of its own configuration. Making it
installable from npm is a one-line change the owner approves, and not one to make in passing. A
`Dockerfile` builds a stdio image for a client that runs `docker run -i`; publishing it to a registry
is the owner's decision as well.
