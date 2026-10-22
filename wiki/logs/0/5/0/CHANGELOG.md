# 0.5.0

**Released:** 2026-10-22

`lxagents-gitlab`'s source is reorganised by role. `src/` was one flat folder of shared modules beside `src/tools/`;
it is now `src/index.js` and six folders: `routes/`, `controllers/` (the former `src/tools/`), `services/`,
`models/`, `utils/` and `config/`. **The 45 tools, their arguments and results, the two tokens, the variables and
the HTTP routes do not change.** A client that starts the server with `node src/index.js`, `npm start`,
`npm run start:http` or the image notices nothing.

**Breaking, for one path.** `src/http.js` could be run directly, and it has moved to `src/routes/http.js`. Nothing
is left at the old path. Anything that imported a module from `src/` by path breaks the same way; the package is
private, so nothing outside this repository should.

**Clients must:**

1. **If you start the HTTP transport with `node src/http.js`,** use `node src/routes/http.js`, `npm run start:http`,
   or `MCP_TRANSPORT=http node src/index.js`.
2. Otherwise nothing. Restart the client as usual to pick up the release.

## Changed

- **`src/routes/`:** `http.js` (the HTTP transport) and `server.js` (the MCP server and tool registration).
- **`src/controllers/`:** every tool module and the registry, renamed from `src/tools/`. A module's file name is
  still its tool's name.
- **`src/services/`:** `api.js`, `auth.js`, and the GitLab calls several tools share: `contents.js`, `users.js`, `merge-status.js`.
- **`src/models/`:** `args.js`, `schemas.js` and `shapes.js`.
- **`src/utils/`:** `render.js`, `text.js`, `diffs.js`, `errors.js`, `log.js`, `wait.js`.
- **`src/config/`:** `config.js` and `version.js`.
- **`npm run start:http`** runs `src/routes/http.js`.
- **The documentation** names the new paths, and the architecture page says what each folder holds.

## Not changed, on purpose

- **No code inside a module changed** beyond its import paths and where `version.js` finds `package.json`.
- **No shim at the old paths.** A second entry point would be a second thing to keep working.
- **`node src/index.js`, `main`, `bin` and the image's `CMD`** stay where they were.

## Added

- **Tests:** none; the same 302 pass under `node --test` as in `0.4.0`, with only their import paths changed.

## Security

- **Nothing changes in how either secret is read, compared, sent or scrubbed,** and the HTTP transport is still the
  only listener, now at `src/routes/http.js`. **No dependency was added.**
