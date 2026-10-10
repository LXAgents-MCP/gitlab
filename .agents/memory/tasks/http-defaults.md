---
name: memory-tasks-http-defaults
description: Giving the GitLab MCP server's HTTP transport the same defaults as shared-instruction, port 3000 on all interfaces; released as 0.4.0.
---

# HTTP defaults — GitLab

## 2026-10-10 — complete

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

**Status:** complete. Released as `0.4.0`; the pull requests, merged in order by rebase, are in the table.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/http-defaults-plan` | [#25](https://github.com/LXAgents-MCP/gitlab/pull/25) |
| 2 | Default to port 3000 on all interfaces | `feat/http-defaults` | [#26](https://github.com/LXAgents-MCP/gitlab/pull/26) |
| 3 | Corrected documentation | `docs/http-defaults` | [#27](https://github.com/LXAgents-MCP/gitlab/pull/27) |
| 4 | Release | `release/0.4.0` | [#28](https://github.com/LXAgents-MCP/gitlab/pull/28) |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. A rebase merge rewrites commits, so
each branch was replayed onto the new `master` before its pull request was merged, in order, by rebase.

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

### Task 2 — feat/http-defaults

Landed. `DEFAULT_PORT` is 3000 and `DEFAULT_HOST` is `0.0.0.0` in `src/config.js` (H1), so with neither variable set
the HTTP transport listens on all interfaces on port 3000, as `shared-instruction` does. `httpSettings` is
otherwise unchanged: unset, empty or blank `PORT` and `HOST` take the defaults, `PORT` and `HOST` override, `PORT=0`
asks the system for a free port, and a `PORT` that is set but not a whole number from 0 to 65535 exits 2 (H2). The
start-up line says `serving over http on :3000/mcp (all interfaces)` when the host is `0.0.0.0`, and the full URL
otherwise. The usage text and the headers of `src/http.js` and `src/index.js` no longer say loopback. The
fail-closed start without a usable `MCP_AUTH_TOKEN` is untouched and is the guard (H4). The decision is recorded in
`.agents/memory/decisions/http-bind-defaults.md`, with what it costs and the one place `PORT` parsing is not copied.

Tests: 302 passing, none failing (300 before). `httpSettings` is checked for the defaults (unset, empty, blank),
for overrides, for `0`, and for five values that are not ports. A real process with no `HOST` is shown to listen
and answer, with the start-up line saying `(all interfaces)`, and with `HOST=127.0.0.1` it says the URL; the port
comes from a free-port probe because the real default, 3000, may be taken on the machine running the tests. Putting
the default host back to loopback fails three tests, which was confirmed. By hand: with `PORT` set and no `HOST`
the server logs `(all interfaces)` and answers `/healthz`, and `PORT=abc` exits 2 with the message.

Left for task 3: every page that says loopback by default or port 3001.

### Task 3 — docs/http-defaults

Landed. Every page that said the HTTP transport binds loopback by default, or named port 3002, now says port 3000
on all interfaces. `wiki/guides/setup-http-token.md` has the start-up line as it prints now, the `PORT` and `HOST`
rows, every curl and client URL on port 3000, a section "Who can reach it" (renamed from "Putting it on a network")
that says plainly that by default the port is open on the machine's network and that the access token is what stands
in front of the key, the container run without `-e HOST=0.0.0.0` and with `-p 127.0.0.1:3000:3000` to keep it to
the host, a refusal row for a port that is taken, and a section "If you used 0.3.0" for the move from `3002` and
loopback. Also corrected: `env.md` (the two rows, with the note that the GitHub server also defaults to 3000),
`setup.md`, `docker.md`, `overview.md`, `security-model.md` (the surfaces row, the deployment posture, the
controls), `smoke-test.md` (the curl ports), `README.md`, and on the agent side `wiki/context/repository-map.md`
and `wiki/security/security-boundaries.md`, which now says the 32-character access token is the only guard and must
stay fail-closed. The released changelogs and their rows are left as written, and so are the sentences about the
test suite reaching no host beyond loopback, which are about the tests.

Checked: no relative link in any `.md` file is broken, and no page outside the released history and the records
names `3002` or loopback as the HTTP default. Code is untouched in this task.

### Task 4 — release/0.4.0

Landed. The version is `0.4.0` in `package.json` and the lockfile's own two entries (H3: chosen under the owner's
instruction to take the recommended option, and stated to them with the plan). `wiki/logs/0/4/0/CHANGELOG.md` says
first that the port and the exposure change, and that the second is the one to read twice, then what clients must do:
change the port in their URLs, narrow the bind with `HOST=127.0.0.1` if they relied on loopback, set `PORT` if the
GitHub server shares the machine, and restart. It also records what was not copied from `shared-instruction`, the
silent `PORT` fallback. Its row is first in `index/logs-index.md`. Earlier changelogs are left as written.
`memory/state/repository-state.md` describes `0.4.0`.

Checked from a clean export of the index, with `npm ci`: 302 tests pass and none fail, and
`node src/index.js --version` prints `lxagents-gitlab 0.4.0`. No git tag was made and nothing was published.

The `PR` column of the table is filled by a closing commit once the pull requests exist, so no earlier branch needs
a rebase.

## Closing

The four pull requests were merged in order by rebase. After the last, `master` was identical in tree to the
original `release/0.4.0` tip, and 302 tests passed from a clean install with `--version` printing `0.4.0`. The merged
branches were not deleted, because the git proxy refuses it; the owner deletes them.
