---
name: memory-tasks-http-defaults
description: Giving the GitLab MCP server's HTTP transport the same defaults as shared-instruction, port 3000 on all interfaces; released as 0.4.0.
---

# HTTP defaults — GitLab

## 2026-10-10 — in progress

**Goal.** The HTTP transport listens where `shared-instruction` listens by default, so the three servers are
reached the same way: `PORT` 3000 and `HOST` `0.0.0.0`. Until now this server defaulted to `127.0.0.1:3002`.

**Objective.** With neither `PORT` nor `HOST` set, `MCP_TRANSPORT=http node src/index.js` listens on port 3000 on
all interfaces and says so at start. `PORT` and `HOST` still override. A `PORT` that is set but is not a port still
exits 2. The fail-closed start without a usable `MCP_AUTH_TOKEN` is unchanged. `npm test` is green and every page that
said loopback or 3002 is corrected.

**Detail.** Edit this repository only; `LXAgents-MCP/github` has the same change in its own record, made before this
one, and its default port is 3000 as well. The 45 tools and the two-token design do not change. The reference is
`shared-instruction`'s `src/http.js`: `const PORT = Number(process.env.PORT) || 3000; const HOST = process.env.HOST || "0.0.0.0";`.
The owner asked for this and said to take the recommended option wherever a choice was open.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/http-defaults-plan` | |
| 2 | Default to port 3000 on all interfaces | `feat/http-defaults` | |
| 3 | Corrected documentation | `docs/http-defaults` | |
| 4 | Release | `release/0.4.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by a closing commit
once the pull requests exist.

## Decisions

| # | Decision |
|---|---|
| H1 | The defaults are `shared-instruction`'s: port 3000, host `0.0.0.0`. They replace D5 and D6 of `http-token-auth.md` (loopback by default; port 3002 so three servers can share a machine). Both forge servers now default to 3000, so running two on one machine needs `PORT` set on at least one. |
| H2 | A `PORT` that is set but is not a whole number from 0 to 65535 still exits 2, where `shared-instruction`'s `Number(PORT) \|\| 3000` would silently listen on 3000. Listening somewhere other than asked is worse than refusing to start. `PORT=0` stays valid and asks the system for a free port, which the tests rely on. Declined: copying the expression exactly. |
| H3 | The release is `0.4.0`. The port and the bind address are what existing clients connect to, so this is a breaking change in a 0.x line. Chosen under the owner's instruction to take the recommended option; stated to them with the plan. No git tag, nothing published. |
| H4 | The guard for binding all interfaces is the one already there: HTTP refuses to start without `MCP_AUTH_TOKEN` of at least 32 characters, and every route but `/healthz` needs it. Nothing is added. |
| H5 | The documentation says what binding all interfaces means and how to narrow it: `HOST=127.0.0.1` on a single machine, `-p 127.0.0.1:3000:3000` for the container, and TLS in front before the port faces a network. The container no longer needs `-e HOST=0.0.0.0`. |

## Task entries

### Task 1 — chore/http-defaults-plan

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes in this task. The `PR`
column is filled by a closing commit, not here, so no later branch needs a rebase.

Baseline before any change: `npm test` ran 300 passing and none failing.
