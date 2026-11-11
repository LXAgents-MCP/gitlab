# 0.6.1

**Released:** 2026-11-11

**Internal.** The two files that still held several jobs after 0.5.0 are split, one job per file.
Nothing a client sees changes: the 45 tools, their arguments, results and messages, the tokens, the
variables, the routes and the local-install rules of 0.6.0 are all as they were. No path anyone runs
moves.

**Clients must:** nothing.

## Modified

| What | Was in | Now in |
|---|---|---|
| Validating and encoding a repository, reference or path | `src/services/api.js` | `src/models/address.js` |
| `pageQuery`, `paging` | `src/services/api.js` | `src/models/paging.js` |
| `callApi`, the request with a timeout and a size cap | `src/services/api.js` | `src/services/client.js` |
| The wording of every failure | `src/services/api.js` | `src/services/failure.js` |
| `requireToken` | `src/services/api.js` | `src/services/auth.js` |
| `redact` | `src/services/api.js` | `src/utils/redact.js` |
| `httpSettings` (`PORT`, `HOST`) | `src/routes/http.js` | `src/config/http-settings.js` |
| `rpcError`, `unauthorized`, `readBody` | `src/routes/http.js` | `src/utils/http.js` |

`src/services/api.js` keeps `gitlab()`, still the only place a GitLab URL is built.

## Verified

`npm test`: 310 passing, none failing, the same as before. No module imports from a folder above its
own. Still nothing run against live GitLab.
