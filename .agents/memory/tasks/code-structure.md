---
name: memory-tasks-code-structure
description: Splitting the two files that still held several jobs after 0.5.0, src/services/api.js and src/routes/http.js, into one job per file, with no change a client can see; done alongside github; released as 0.6.1.
---

# Code structure — GitLab

## 2026-11-11 — in progress

**Goal.** The owner asked for every repository to be checked against `CODE_PERF.md`: one job per
file, no tangled dependencies, a layout that scales, no redundant logic. The folders are already right
since 0.5.0, but that change was a move and not a split, and two files still hold several jobs.
`src/services/api.js` holds the token check, argument validation and URL encoding, paging, the fetch
client and the mapping of a status to a message. `src/routes/http.js` holds settings parsing, the
response helpers and body reader, the request handler and the process lifecycle, and the two route
files import `redact` from the API client.

**Objective.** One job per file:

| Path | Holds |
|---|---|
| `src/services/api.js` | `gitlab()` and `branchOrDefault`: the one place a URL is built and a call made. |
| `src/services/client.js` | `callApi`: one request with a timeout and a size cap. |
| `src/services/failure.js` | `apiDetail` and `failure`: a status becomes a message that names the next step. |
| `src/services/auth.js` | Gains `requireToken`, beside the key it checks. |
| `src/models/address.js` | `parseRepo`, `refName`, `refPath`, `filePath`, `isNamespacePath`, `SEGMENT`: validation and URL encoding of path parts. |
| `src/models/paging.js` | `pageQuery`, `paging`. |
| `src/utils/redact.js` | `redact`. |
| `src/utils/http.js` | `rpcError`, `unauthorized`, `readBody`. |
| `src/config/http-settings.js` | `httpSettings`: `PORT` and `HOST`. |
| `src/routes/http.js` | `localRequestProblem`, the request handler, `startHttpServer`, `main`. |

Imports still only flow downward: routes, controllers, services, models, utils, config. `npm test`
passes with the same 310 tests.

**Detail.** A split, not a rewrite: no tool, argument, description, variable, route, status or message
changes. Declarations move whole; only their imports, an `export` keyword and the API client's header
comment change. No shim is left at an old place. Done after the 0.6.0 auth-token chain merged, since it
edited the same files. The owner is away and asked for the recommended option wherever a question comes
up. Baseline: 310 passing.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `plan/code-structure` | |
| 2 | Split api.js and http.js by job | `refactor/code-structure` | |
| 3 | Corrected documentation | `docs/code-structure` | |
| 4 | Release | `release/0.6.1` | |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. The same names are used in `github`,
`cli` and `template`; no chain depends on another.

## Decisions

| # | Decision |
|---|---|
| D1 | Validation of path parts is input modelling, so it moves to `models/`; `api.js` stays the only file that builds a URL, so the rule "every URL is built in `src/services/api.js`" still holds. |
| D2 | `localRequestProblem` stays in `routes/http.js`: it is the transport's own admission rule and needs `isLoopbackHost` from `services/`, which `utils/` may not import. |
| D3 | The code `github` and `gitlab` duplicate (`callApi`, `redact`, the HTTP transport) is not moved to a shared package: that adds a published dependency and its own release pipeline, a decision of its own. |
| D4 | The release is `0.6.1`, a patch: no path anyone runs moves. No tag, nothing published. |

## Task entries

### Task 1 — plan/code-structure

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes. The `PR`
column is filled by the release task.

### Task 2 — refactor/code-structure

Landed. A script moved each declaration whole, by name, with its leading comment, and wrote each new
module's imports from what its code uses; every import of a moved name in `src/` and `test/` was
rewritten to its new home. From `src/services/api.js`: path-part validation to `src/models/address.js`,
`pageQuery` and `paging` to `src/models/paging.js`, `readText` and `callApi` to `src/services/client.js`,
the status wording to `src/services/failure.js`, `redact` to `src/utils/redact.js` and `requireToken` to
`src/services/auth.js`. From `src/routes/http.js`: `httpSettings` to `src/config/http-settings.js`,
`rpcError`, `unauthorized` and `readBody` to `src/utils/http.js`. `api.js` gets a header that says what it
now holds; two comments that named the old home of a function name the new one (D1). No code inside a
function changed.

Checked: no module imports from a folder above its own, every relative import resolves, and no import is
unused. `npm test` runs 310 passing and none failing, the same as before.

Left for task 3: the rules, the repository map, the security boundaries and the wiki pages name
`src/services/api.js` for validation, and the architecture page lists the files of each folder.
