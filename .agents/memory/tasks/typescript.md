---
name: memory-tasks-typescript
description: Migrating the server and its tests from plain JavaScript to TypeScript, compiled to dist/ with tsc, with dependencies on their latest versions and a clean npm audit; done alongside github, cli and template.
---

# TypeScript migration — GitLab

## 2026-11-11 — in progress

**Goal.** The owner asked for every Node.js repository to move to TypeScript, so the code carries its
types in the language rather than in JSDoc, and a type error is caught by the compiler before a test or a
client finds it.

**Objective.**

* Every dependency at its latest version; `typescript`, `@types/node` and `tsx` as dev dependencies;
  `npm audit` reports nothing.
* `tsconfig.json` for Node ESM: `target` `ES2022`, `module` and `moduleResolution` `NodeNext`,
  `rootDir` `./src`, `outDir` `./dist`, `strict` on.
* Every `.js` file in `src/` and `test/` is `.ts`, with types on functions, variables and the MCP SDK
  objects, and no implicit `any`.
* `package.json`: `main` and `bin` point at `dist/index.js`; `build` is `tsc`; `start` runs
  `node dist/index.js`; `test` is `node --import tsx --test test/*.test.ts`.
* `npm run build` compiles with no error, `npm test` passes with the same tests as before, and a final
  `npm audit` is clean.

**Detail.** A conversion, not a rewrite: no tool, argument, description, variable, route, status or
message changes, and each file keeps its one job. `dist/` is build output and is ignored, never committed.
The repository rule "no build step, no TypeScript" is set aside by the owner's request; the documentation
that states it is corrected in task 4. The owner is away and asked for the recommended option wherever a
question comes up. Baseline: 310 passing; `npm audit` reports body-parser (high), fast-uri, ip-address, qs
and zod (moderate), all transitive or fixable in range.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `plan/typescript` | |
| 2 | Latest dependencies and a clean audit | `build/dependencies` | |
| 3 | Convert src/ and test/ to TypeScript | `refactor/typescript` | |
| 4 | Corrected documentation | `docs/typescript` | |
| 5 | Release | `release/0.7.0` | |

Branches stacked: task 1 from `master`, task `k` from task `k-1`. The same chain runs in `github`, `cli`
and `template`; no chain depends on another.

## Decisions

| # | Decision |
|---|---|
| T1 | The build is `tsc` alone, into `dist/`; no bundler. Tests run the sources through `tsx`, so a test never depends on a stale build. |
| T2 | Relative imports keep their `.js` extension, which `NodeNext` resolves to the `.ts` source and which is what the compiled file needs at run time. |
| T3 | A GitLab response body is external JSON, typed at the one place it enters (`services/api.js`) rather than given a hand-written interface per endpoint that nothing would check. |
| T4 | The tool registry discovers `.ts` files when run from source and `.js` files when run from `dist/`, so the same module serves tests and production. |
| T5 | The image builds in two stages: one with the dev dependencies to run `tsc`, one with only the production tree and `dist/`. |
| T6 | `@types/express` is not installed: the server does not import Express; only the MCP SDK does, and it ships its own types. |
| T7 | The release is `0.7.0`: no tool changes, but the file a client runs moves from `src/index.js` to `dist/index.js` after `npm run build`. No tag, nothing published. |

## Task entries

### Task 1 — plan/typescript

Landed: this record and its row in `memory-index.md`. Nothing outside `.agents/` changes. The `PR`
column is filled by the release task.

### Task 2 — build/dependencies

Landed. `@modelcontextprotocol/sdk` to `^1.32.2` and `zod` to `^4.6.7`, their latest; `typescript`
(`^7.0.2`), `@types/node` (`^26.6.6`) and `tsx` (`^4.23.15`) added as dev dependencies, which the image's
`--omit=dev` install leaves out. `npm audit fix` moved body-parser, fast-uri, ip-address and qs inside the
lockfile; `npm audit` now reports 0 vulnerabilities and `npm outdated` nothing. No source file changes, and
`npm test` still runs 310 passing.

### Task 3 — refactor/typescript

Landed. Every `.js` file in `src/` and `test/` is renamed to `.ts` (`git mv`, so history follows), and
`tsconfig.json` compiles `src/` to `dist/` (`ES2022`, `NodeNext`, `strict`); `tsconfig.test.json`
type-checks `src/` and `test/` together without emitting, run by `npm run typecheck`. Both pass with no
error and no implicit `any`.

* Types are in the code and the JSDoc types are gone; the descriptions they carried stay.
  New named types: `ToolDefinition`, `ToolOutcome`, `ToolHandler` and `ToolSpec` in
  `controllers/index.ts`; `Repo` in `models/address.ts`; `PageQuery` and `Paging`; `IntRange` and
  `ListOptions` in `models/args.ts`; `ApiAnswer` and `CallOptions` in `services/client.ts`;
  `GitlabOptions` in `services/api.ts`; `FailedCall`, `Mergeability`, `FoundUser`; `HttpOptions` and
  `RunningServer` in `routes/http.ts`; the `*Fields` result types in `models/shapes.ts`. The MCP SDK's
  own `ToolAnnotations` and `CallToolResult` type the hints and results.
* `models/api-body.ts` holds `ApiBody`, the one loose type: JSON from GitLab (T3).
* `rawText` gained overloads, so a call with `required: true` is typed `string`. No body changed.
* The registry finds `.ts` modules under tsx and `.js` modules in `dist/` (T4).
* `package.json`: `main` and `bin` are `dist/index.js`, `files` ships `dist`, `build` is `tsc`, `start`
  and `start:http` run `dist/`, `test` is `node --import tsx --test test/*.test.ts`, `inspect` runs
  `dist/index.js`. `--help` names `node dist/index.js`.
* `dist/` is in `.gitignore` and `.dockerignore`. The `Dockerfile` builds in two stages and runs
  `node dist/index.js` (T5); it has not been built, as there is no Docker daemon here.
* The process tests spawn `node --import tsx src/index.ts`.

Checked: `npm run build` and `npm run typecheck` pass; `npm test` runs 308 tests, 308 passing. The count
is 2 lower than 310 only because the glob no longer runs `test/helpers.ts` and `test/surface.ts`, which
`node --test` used to count as two passing files with no tests in them. The compiled server answered
`--version`, listed 45 tools over a real stdio pipe, refused `repo_read` for a missing key, and served
`GET /healthz` over HTTP from `dist/routes/http.js`.

Left for task 4: every page that says plain JavaScript, no build step, `node src/index.js` or a `.js`
path under `src/`.

### Task 4 — docs/typescript

Landed. Every living page that said plain JavaScript, no build step, `node src/index.js` or a `.js` path
under `src/` or `test/` now says TypeScript, `npm run build`, `node dist/index.js` and the `.ts` path:
`README.md`; the repository rules (the stack line, which allowed no TypeScript, now says the build is
`tsc` alone; the commands table gains `build` and `typecheck`; the conventions gain types in code and
`ApiBody`), the tool-surface rule, the repository map (entry point, commands, `dist/` as generated, the
image row, an `api-body.ts` row) and the security boundaries; in `wiki/`, setup, Docker (two stages, and
what was checked without a daemon), connect a client, HTTP setup, environment, smoke test, architecture,
reference and security model. A client config's `args` names `dist/index.js`. Released changelogs and
earlier task records and decisions keep the old paths.

Checked before writing the Docker page: each image stage's steps run in a scratch directory with only
what that stage copies, and the production tree with `dist/` served all 45 tools over stdio.

### Task 5 — release/0.7.0

Landed. `package.json` and the lockfile are at `0.7.0`, a minor release (T7); the release log is
`wiki/logs/0/7/0/CHANGELOG.md` and the logs index leads with its row; the repository state says
TypeScript. Final checks on this branch: `npm run build` and `npm run typecheck` pass, `npm test` runs
308 passing and none failing, `npm audit` finds 0 vulnerabilities, and `node dist/index.js --version`
prints `lxagents-gitlab 0.7.0`. No tag is made and nothing is published.
