---
name: memory-tasks-local-auth-token
description: Letting a local install of this server's HTTP transport run without MCP_AUTH_TOKEN, while a server install still requires it; done alongside LXAgents-MCP/github.
---

# Access token only for server installs — GitLab

## 2026-11-11 — complete

**Goal.** A local install should not need `MCP_AUTH_TOKEN`; only a server install should. stdio already never
reads it, but the HTTP transport refuses to start without it even when nothing beyond this machine can reach it,
so running the server over HTTP on a laptop means inventing a 32-character secret for nobody.

**Objective.** HTTP that only this machine can reach starts and serves without `MCP_AUTH_TOKEN`. HTTP that other
machines can reach still refuses to start without a usable one, exactly as now. A token that is set is always
checked and always required on every request, wherever the server listens. `npm test` is green and every page
that says HTTP always needs the token is corrected.

**Detail.** The owner asked for this in `gitlab` and `github` (not `cli`, which has no HTTP transport;
`template` never reads `MCP_AUTH_TOKEN`, so nothing there changes). No new variable: whether an install is local
is read from where it listens. The 45 tools, `GITLAB_API_KEY`, the routes and stdio do not change. The owner
said to plan and proceed, open the pull requests, and merge them in order by rebase, taking the recommended
option wherever a choice was open; the choices are below. Baseline before any change: `npm test` ran 302
passing and none failing.

**Status:** complete. Released as `0.6.0`; the pull requests, merged in order by rebase, are in the table.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `plan/local-auth-token` | [#34](https://github.com/LXAgents-MCP/gitlab/pull/34) |
| 2 | No access token on loopback | `feat/local-http` | [#35](https://github.com/LXAgents-MCP/gitlab/pull/35) |
| 3 | Corrected documentation and rules | `docs/local-http` | [#36](https://github.com/LXAgents-MCP/gitlab/pull/36) |
| 4 | Release | `release/0.6.0` | [#37](https://github.com/LXAgents-MCP/gitlab/pull/37) |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. The same names are used in
`LXAgents-MCP/github`; neither chain depends on the other, so they merge independently.

## Decisions

| # | Decision |
|---|---|
| L1 | **Local means loopback.** An install is local when the HTTP transport listens on a loopback address (`127.0.0.0/8`, `::1` or `localhost`); anything else, `0.0.0.0` included, is a server install. Declined: a new variable such as `MCP_INSTALL=local`, which the repository rules forbid without the owner's say-so and which could claim "local" while listening everywhere. |
| L2 | **With `HOST` unset, the bind follows the token.** `MCP_AUTH_TOKEN` set: `0.0.0.0`, as today. Unset: `127.0.0.1`. So a local `npm run start:http` needs no variable at all, and every configuration that works today keeps its bind. Declined: changing the default to loopback for everyone, which would move servers that have a token and rely on the `0.0.0.0` default; and keeping `0.0.0.0` for everyone, which would leave a local install needing `HOST=127.0.0.1` to skip the token. |
| L3 | **No token and a non-loopback `HOST` still exits 1**, naming `MCP_AUTH_TOKEN` and saying that a local install can leave `HOST` unset. A server install is never open. |
| L4 | **A token that is set is always enforced**, on loopback too, and a set token under 32 characters still exits 1. Setting it is how a local install opts back in. |
| L5 | **Without a token, the `Host` header must name loopback, and an `Origin` header, if sent, must too.** Otherwise `403`, before the body is read. A loopback port with no token is reachable from any web page the person opens, through DNS rebinding; this is the guard that the token was. `GET /healthz` stays open, as it is today. |
| L6 | **The release is `0.6.0`.** New behaviour, and nothing that works today stops working, so a minor release in the 0.x line. Chosen under the owner's instruction to take the recommended option. No git tag, nothing published. |
| L7 | **Two rules are changed, on the owner's instruction.** `.agents/rules/repository.md` and `.agents/wiki/security/security-boundaries.md` forbid HTTP starting without an access token. The owner asked for exactly that on loopback, so both are corrected to say "on an address other machines can reach". |

## Task entries

### Task 1 — plan/local-auth-token

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes in this task. The `PR`
column is filled by the release task, so no later branch needs a rebase.

### Task 2 — feat/local-http

Landed. `src/routes/http.js` tells a local install from a server install by where it listens (L1): with no
`MCP_AUTH_TOKEN` and a loopback `HOST`, `main()` and `startHttpServer` start without a token; with any other `HOST`
they refuse as before, and `main()` now names the address and says a local install can leave `HOST` unset (L3). A
set token is checked and required wherever the server listens (L4). `httpSettings` defaults `HOST` to `0.0.0.0` when
the token is set and to `LOCAL_HOST`, `127.0.0.1`, a new constant in `src/config/config.js`, when it is not (L2).
Without a token, `localRequestProblem` refuses with `403` any request whose `Host`, or `Origin` when sent, is not
loopback, before the body is read (L5); `isLoopbackHost` in `src/services/auth.js` matches the literal names only,
by hand, so `src/routes/http.js` is still the only module that loads a network module. The start-up line says
`local install` and what that means. `src/index.js`'s usage text and the module headers say the same. The decision
is `.agents/memory/decisions/local-install-without-token.md`.

Tests: 310 passing, none failing (302 before). New: the two `HOST` defaults; a start refused on `0.0.0.0` and on
another address without a token, and with a short token on loopback; a local install serving `tools/list` with no
token for every loopback `Host` form and a loopback `Origin`; refusals for a foreign, rebound or look-alike `Host` and
a foreign, `null` or `file:` `Origin`, before the body is read; routing unchanged after the check; and a real process
with neither variable set serving on `127.0.0.1` and saying `local install`. Turning the `Host` check off fails a
test, which was tried. By hand: with neither variable the process served `tools/list` to curl on `localhost` and
refused `Host: evil.example` with `403`; with `HOST=0.0.0.0` and no token it exited 1 with the new message.

Left for task 3: every page and rule that says HTTP always needs the token.

### Task 3 — docs/local-http

Landed. Every living page that said HTTP always needs `MCP_AUTH_TOKEN` now tells a local install from a server
install: the README (transport, tokens, and a local quick start), `wiki/environments/` (`env.md`'s `MCP_AUTH_TOKEN` and
`HOST` rows, `setup.md`, and `docker.md`, which says a container is a server install), `wiki/guides/setup-http-token.md`
(a new section "Local install or server install", the variable table, the start-up refusal, and three new rows in
"When something is wrong"), `wiki/guides/smoke-test.md` (H1 on `0.0.0.0`, and a new H1b for a local install),
`wiki/information/` (overview and architecture), `wiki/reference/mcp-surface.md` and `wiki/security/security-model.md`
(the listener row, the transport section and two verification rows). The two rules that forbade HTTP starting without
a token, `.agents/rules/repository.md` and `.agents/wiki/security/security-boundaries.md`, now forbid it on an
address that is not loopback, and forbid a local install without the `Host` and `Origin` checks (L7);
`.agents/wiki/context/repository-map.md` and the state file say the same. Released changelogs and earlier task
records are left as written.

Left for task 4: the version, the release log, the logs index row and the `PR` column.

### Task 4 — release/0.6.0

Landed. `package.json` and the lockfile are `0.6.0` (L6): new behaviour, and nothing that worked in `0.5.0` stops
working, so a minor release in the 0.x line. The release log is `wiki/logs/0/6/0/CHANGELOG.md`, with what a client
must do (nothing, unless it wants a local install without a token; keep the token in a container), and the logs
index has its row. The state file names the version and the test count. No git tag is made and nothing is
published. The `PR` column is filled in a closing commit on this branch once every pull request is open.
