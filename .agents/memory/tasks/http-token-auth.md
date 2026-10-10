---
name: memory-tasks-http-token-auth
description: Giving the GitLab MCP server an HTTP transport that requires a bearer token, and renaming its one variable to MCP_AUTH_TOKEN; released as 0.2.0.
---

# Token over HTTP — GitLab

## 2026-10-10 — in progress

**Goal.** A client can reach the GitLab server the way it reaches `shared-instruction` over HTTP: a
required bearer token in the `Authorization` header, set in the client's configuration, instead of a
secret that has to be in the server's own environment.

**Objective.** `MCP_TRANSPORT=http node src/index.js` serves `POST /mcp` and `GET /healthz`. Every route
but `/healthz` needs `Authorization: Bearer <GitLab token>`, answers 401 without it, and uses that token
for the GitLab calls of that request only. The one variable is `MCP_AUTH_TOKEN`, the name
`shared-instruction` uses for its token; stdio reads the GitLab token from it, HTTP never does. The old
`LXAGENTS_MCP_GITLAB_API_KEY` is gone everywhere. `npm test` is green and a wiki page says how to set it up.

**Detail.** Edit this repository only; `LXAgents-MCP/github` has the same change in its own record, made before
this one. The 45 tools do not change. No new dependency: the SDK's streamable-HTTP transport runs on
`node:http`. The reference is `shared-instruction`'s `src/auth.js` and `wiki/environments/env.md`. The
version bump to `0.2.0` was approved with the plan.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/http-token-auth-plan` | |
| 2 | HTTP transport with a bearer token | `feat/http-token-auth` | |
| 3 | Setup guide and corrected documentation | `docs/http-token-setup` | |
| 4 | Release | `release/0.2.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by a closing
commit once the pull requests exist.

## Decisions the owner approved with the plan

| # | Decision |
|---|---|
| D1 | Over HTTP the client's bearer token is the GitLab token itself, passed through per request. The server stores no secret. Declined: a separate server access token beside a GitLab token in the environment, which is two secrets and still an environment. |
| D2 | stdio stays, because a pipe has no header to carry a token, and reads the GitLab token from `MCP_AUTH_TOKEN`. The old `LXAGENTS_MCP_GITLAB_API_KEY` is deleted, not kept as a second name. Declined: removing stdio, which breaks every registration and `docker run -i`. |
| D3 | HTTP never reads the environment variable. An environment token is not a fallback, or a caller with no token would act with the owner's. |
| D4 | The token is checked for shape only: not empty, no whitespace, at most 512 characters. `shared-instruction` refuses under 32, which would refuse a GitLab token (26 characters), so the floor is not copied. |
| D5 | The default bind is `127.0.0.1`, because what crosses the wire is a GitLab credential. `HOST` overrides it. `shared-instruction` binds `0.0.0.0`. |
| D6 | The variables are `MCP_TRANSPORT`, `PORT` and `HOST`, named as in `shared-instruction`. The default port is `3002`, so the three servers can run on one machine. |
| D7 | No dependency is added: `node:http` and the SDK's transport. Declined: Express. |
| D8 | The release is `0.2.0`, approved. No git tag is created and nothing is published without its own approval. |
| D9 | Standing instruction: where a choice is needed, use the recommended option and add it to this table. |
| D10 | The variable is named `MCP_AUTH_TOKEN` in both forge servers, by the owner's instruction, the name `shared-instruction` uses for a different secret. A value exported in a shell for one would be inherited by the other's stdio process and sent to GitLab, so the documentation says to set it only in a server's own `env` block. |

## Task entries

### Task 1 — chore/http-token-auth-plan

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes in this task.
The `PR` column is filled by a closing commit, not here, so no later branch needs a rebase.

Baseline before any change: `npm test` ran 272 passing and none failing.

### Task 2 — feat/http-token-auth

Landed. `MCP_TRANSPORT=http node src/index.js`, or `npm run start:http`, serves `POST /mcp` and `GET /healthz`
on `127.0.0.1:3002`, with `PORT` and `HOST` to change them. Every route but `/healthz` needs
`Authorization: Bearer <GitLab token>`: a missing or malformed header is a 401 in the JSON-RPC envelope with
`WWW-Authenticate: Bearer realm="mcp"`, and a credential longer than 512 characters or not printable ASCII adds
`error="invalid_token"`. The check comes before the body is parsed and before a path is matched, so a caller
without a token cannot make the server read 4 MB or find out which routes exist; with a token, an unknown path is
404, a method other than POST is 405 with `Allow: POST`, a body that is not JSON is 400 and one over 4 MB is 413.
Each request gets its own `McpServer` and a stateless streamable-HTTP transport, closed with its response.

The variable is renamed: `LXAGENTS_MCP_GITLAB_API_KEY` is gone from `src/`, the tests and the tool descriptions,
and `MCP_AUTH_TOKEN` replaces it (D2, D10). The token source is one place, `src/auth.js`: the request's bearer
token reaches `requireToken` through an `AsyncLocalStorage`, and a process that serves HTTP is marked by
`enterHttpMode`, after which the environment is never consulted (D3). That mark is the guard, not the request
context: if the context were ever lost, a tool would be refused, not run with the environment's token. The 401
wording from GitLab and the crash scrubbing follow the active source, so over HTTP they name the header and
scrub the request's token. GitLab receives the token as `Authorization: Bearer`, as before. A process that
serves HTTP with `MCP_AUTH_TOKEN` also set says on stderr that it is ignored, without the value. A bad
`MCP_TRANSPORT` or `PORT` exits 2.

`node:http` appears only in `src/http.js`; the dependencies are still the MCP SDK and zod (D7). The decision is
recorded in `.agents/memory/decisions/http-token-transport.md`, which answers the objections of the stdio-only
record one by one, and that record is marked superseded and kept.

Tests: 292 passing, none failing; 20 are new. `test/auth.test.js` covers the header parser, the shape check (a
26-character `glpat-` token passes, which is why there is no floor), the sources and their precedence, 12
overlapping requests each keeping their token, and HTTP mode ignoring the environment. `test/http.test.js` runs
over real loopback sockets with `node:http` as the client, because `fetch` is what stands in for GitLab: the 401
cases, the exact `/healthz` exemption, 404/405/400/413, 45 tools listed, the request's token reaching GitLab with
the environment's set, no upstream call without a token, 8 concurrent tokens not mixing, GitLab's rejection naming
the header with the token absent from the response and the log, and a real process that ignores a leftover
`MCP_AUTH_TOKEN`, answers, and stops on SIGTERM with nothing on stdout. Replacing the request-scoped token with
none makes three of them fail, which was confirmed. By hand against a running server: `/healthz` answers, a
request with no token is a 401, and `tools/list` with a token lists 45 tools.

Left for task 3: the setup guide and every page that still says stdio only. The rules `repository.md` and the
security procedure still forbid a listener until task 3 corrects them.

### Task 3 — docs/http-token-setup

Landed. `wiki/guides/setup-http-token.md` is the setup guide: when to choose HTTP over stdio, creating the
token, `npm run start:http`, checking it with curl, registering a client with `claude mcp add --transport http`
or a committed `.mcp.json` that expands a variable of the client's own, putting it on a network behind TLS,
running it in a container, migrating from the old variable, and a table of what each refusal means. It is
linked from the wiki index, `connect-a-client.md`, `setup.md`, `env.md` and `docker.md`.

Corrected, because they said stdio only or named the old variable: `env.md` (now four variables and a table of
where the token comes from), `setup.md`, `connect-a-client.md`, `docker.md` (an "Over HTTP" section, and why
there is still no `EXPOSE` or compose file), `architecture.md` (two transports, where the token comes from),
`overview.md`, `mcp-surface.md`, `security-model.md` (surfaces, secrets, deployment posture, verification rows),
`smoke-test.md`, `README.md`, the intro of `AGENTS.md`, and the comments of the `Dockerfile`. On the agent side:
`rules/repository.md`, `wiki/context/repository-map.md` (rows for `src/http.js` and `src/auth.js`),
`wiki/security/security-boundaries.md`, `memory/state/repository-state.md` and `index/project-wiki-index.md`.

The name sharing of D10 is stated in `env.md` and in the guide: `MCP_AUTH_TOKEN` is set only in a server's own
`env` block, and a client's `headers` expand a variable of the client's own (`${GITLAB_TOKEN}`), never
`MCP_AUTH_TOKEN`. The guide's default port is 3002, the GitHub server's is 3001 and `shared-instruction`'s is 3000.

Checked: no relative link in any `.md` file is broken, and the old variable survives only in the released
`0.1.0` changelog and logs-index row, in the records of earlier work, and in the migration notes that name it.
Code is untouched in this task, so `npm test` is as task 2 left it.
