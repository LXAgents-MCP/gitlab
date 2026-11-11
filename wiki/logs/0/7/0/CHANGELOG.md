# 0.7.0

**Released:** 2026-11-11

**The server is TypeScript.** `src/` and `test/` are `.ts`, type-checked under `strict`, and `tsc` compiles
`src/` to `dist/`, which is what runs. The 45 tools, their arguments, results and messages, the tokens,
the variables, the routes and the local-install rules are all as they were. What moves is the file a
client runs: `node src/index.js` is now `node dist/index.js`, after `npm run build`.

**Clients must:** run `npm install` and `npm run build` in the checkout, change the server's `args` from
`src/index.js` to `dist/index.js` (and `src/routes/http.js` to `dist/routes/http.js` if you run that file
directly), and restart the client. `npm start`, `npm run start:http` and the image already run `dist/`.

## Added

* `tsconfig.json` (`ES2022`, `NodeNext`, `strict`, `src/` to `dist/`) and `tsconfig.test.json`, which
  type-checks `src/` and `test/` together.
* `npm run build` (`tsc`) and `npm run typecheck`.
* Dev dependencies `typescript` 7.0.2, `@types/node` 26.6.6 and `tsx` 4.23.15. None reaches the image.
* Named types for what was JSDoc: `ToolDefinition`, `ToolOutcome`, `ToolSpec`, `Repo`, `ApiAnswer`,
  `GitlabOptions`, `RunningServer` and the rest, and `ApiBody` for an answer from GitLab.

## Changed

* Every `.js` file in `src/` and `test/` is `.ts`. No function body changed beyond types.
* `main` and `bin` are `dist/index.js`; the package `files` ship `dist/`; `start`, `start:http` and
  `inspect` run `dist/`; `--help` says `node dist/index.js`.
* `npm test` is `node --import tsx --test test/*.test.ts`: it runs the sources and needs no build.
* The tool registry loads `.ts` modules from source and `.js` modules from `dist/`.
* The `Dockerfile` builds in two stages; the image holds the production tree and `dist/` only, and runs
  `node dist/index.js`.
* `@modelcontextprotocol/sdk` 1.32.2 and `zod` 4.6.7, their latest.

## Security

* `npm audit` reports 0 vulnerabilities. It reported body-parser (high, request-body denial of
  service), fast-uri, ip-address, qs and zod (moderate), all cleared by the updates above and
  `npm audit fix`.

## Verified

`npm run build` and `npm run typecheck` pass with no error. `npm test`: 308 passing, none failing — the
same tests as 0.6.1's 310, which counted `test/helpers.js` and `test/surface.js` as two files with no
tests. The compiled server listed all 45 tools over a real stdio pipe from the production-only tree, and
served `GET /healthz` over HTTP. The image has still not been built, and still nothing has run against
live GitLab.
