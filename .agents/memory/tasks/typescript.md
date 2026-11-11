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
