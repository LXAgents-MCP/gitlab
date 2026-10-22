---
name: memory-tasks-src-architecture
description: Splitting this server's flat src/ into routes, models, services, controllers, utils and config, with src/tools/ renamed src/controllers/; done alongside LXAgents-MCP/github.
---

# Modular src architecture — GitLab

## 2026-10-22 — in progress

**Goal.** `src/` is one flat folder of shared modules beside `src/tools/`. The owner wants it split by role so
it scales: transport and protocol routing, input models, the GitLab client and the secrets, the tool handlers,
helpers, and configuration each get a folder, the same way in this repository and in `LXAgents-MCP/github`.

**Objective.** `src/` holds `index.js` and six folders: `routes/` (`http.js`, `server.js`), `models/`
(`args.js` and the other zod and mapping modules), `services/` (`api.js`, `auth.js` and the modules that call
GitLab for several tools), `controllers/` (what was `src/tools/`), `utils/` (`render.js`, `text.js`, `errors.js`
and the other helpers) and `config/` (`config.js`, `version.js`). Every relative import resolves, `npm test`
passes with the same count as before, `node src/index.js` and `npm run start:http` still start the server, and
the living documentation names the new paths.

**Detail.** A move, not a rewrite: no tool, argument, description, token, variable or route changes, and no
code inside a module changes beyond its import specifiers and the path it reads `package.json` from. The owner
asked for the two repositories to be done at the same time with background processes, and to take the
recommended answer wherever a question came up; the choices are in the table below. Baseline before any change:
`npm test` ran 302 passing and none failing.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/src-architecture-plan` | |
| 2 | Move src into domain folders | `refactor/src-architecture` | |
| 3 | Corrected documentation | `docs/src-architecture` | |
| 4 | Release | `release/0.5.0` | |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. The same names are used in
`LXAgents-MCP/github`; neither chain depends on the other, so they merge independently.

## Decisions

| # | Decision |
|---|---|
| A1 | Files the request did not name go where their role puts them: `schemas.js` and `shapes.js` to `models/` (zod schemas and the functions that fill them); `contents.js`, `users.js`, `merge-status.js` to `services/` (shared GitLab calls and answers); `log.js`, `wait.js` to `utils/`. The registry `index.js` moves with the tools to `controllers/`. Taken as the recommended answer under the owner's instruction. |
| A2 | Nothing is left at the old paths: no re-export shim at `src/http.js` or `src/tools/`. `npm run start:http` now runs `node src/routes/http.js`; `node src/index.js`, `main`, `bin` and the image's `CMD` do not change. |
| A3 | The release is `0.5.0`. The tools are unchanged, but `src/http.js` was a path a person could run directly and it moves, which in a 0.x line is a minor bump. Chosen under the owner's instruction to take the recommended option. No git tag, nothing published. |
| A4 | The two repositories run concurrently, as the owner asked; inside each, tasks still run in order 1…4 on stacked branches. That sets aside the plan creator's "never work two tasks in parallel" across repositories only, where the two chains share no file. |
| A5 | Released changelogs, earlier task records and decisions keep the old paths: they describe the tree as it was. Repository paths that are tool data — the example `'src/tools'` in `directory_list`'s description and paths inside test fixtures — are not this layout and stay. |

## Task entries

### Task 1 — chore/src-architecture-plan

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes in this task. The `PR`
column is filled by the release task once the pull requests exist, so no earlier branch needs a rebase.
