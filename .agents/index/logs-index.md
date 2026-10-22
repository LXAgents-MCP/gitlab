---
name: logs-index
description: This repository's release history, newest first — what changed and what clients must do.
---

# Logs Index

**Scope:** `wiki/logs/`
**Parent:** [`root-index.md`](root-index.md)

A client picks up a change when it next starts the server, so the **Clients must** column is the
only notice it gets. It is never left blank.

## Versions

| Version | Date | Summary | Clients must |
|---|---|---|---|
| [`0/5/0`](../../wiki/logs/0/5/0/CHANGELOG.md) | 2026-10-22 | **`src/` is organised by role.** `index.js` stays; the rest moves to `routes/`, `controllers/` (was `src/tools/`), `services/`, `models/`, `utils/` and `config/`. The 45 tools, the tokens, the variables and the routes are unchanged; no code changed beyond import paths. **Breaking for one path:** `src/http.js` is now `src/routes/http.js`, with no shim. Still nothing run against live GitLab. | **If you run `node src/http.js`, run `node src/routes/http.js`** (or `npm run start:http`, or `MCP_TRANSPORT=http node src/index.js`). Otherwise nothing. Restart the client. |
| [`0/4/0`](../../wiki/logs/0/4/0/CHANGELOG.md) | 2026-10-10 | **HTTP defaults are `shared-instruction`'s.** `PORT` defaults to 3000 (was 3002) and `HOST` to `0.0.0.0` (was `127.0.0.1`), so the port is open on the machine's network by default; the 32-character `MCP_AUTH_TOKEN` is still required to start and still guards every route. **Breaking:** the port and the exposure change. A set-but-invalid `PORT` still exits 2. Tools and tokens are unchanged. Still nothing run against live GitLab. | **Change client URLs from port 3002 to 3000** (or start with `PORT=3002`). **To keep it to one machine, set `HOST=127.0.0.1`** (container: `-p 127.0.0.1:3000:3000`, and drop `-e HOST=0.0.0.0`). If the GitHub server runs on the same machine, set `PORT` on one of them. Restart the client. |
| [`0/3/0`](../../wiki/logs/0/3/0/CHANGELOG.md) | 2026-10-10 | **Two secrets.** `MCP_AUTH_TOKEN` is now the server's own access token: over HTTP a client must send it as `Authorization: Bearer <token>` (at least 32 characters; HTTP will not start without it). `GITLAB_API_KEY` is the GitLab token, used for every call on stdio and HTTP, never supplied by a client. **Breaking:** `MCP_AUTH_TOKEN` held the GitLab token in 0.2.0. Everyone holding the access token acts as the key's owner. The 45 tools are unchanged. Still nothing run against live GitLab. | **Over stdio, move the GitLab token from `MCP_AUTH_TOKEN` to `GITLAB_API_KEY`** (the `env` block, or `docker run -e`). **To serve HTTP, start it with `GITLAB_API_KEY` and a new `MCP_AUTH_TOKEN`** of 32+ characters (`openssl rand -hex 32`), one per server, and register clients with `Authorization: Bearer <MCP_AUTH_TOKEN>`, not a GitLab token. Restart the client. |
| [`0/2/0`](../../wiki/logs/0/2/0/CHANGELOG.md) | 2026-10-10 | **HTTP, with the GitLab token as the bearer token.** `MCP_TRANSPORT=http` serves `POST /mcp` and `GET /healthz` on `127.0.0.1:3002`; every request carries `Authorization: Bearer <GitLab token>`, the server stores no secret and HTTP never reads the environment. The token variable is renamed `MCP_AUTH_TOKEN`; `LXAGENTS_MCP_GITLAB_API_KEY` is removed. The 45 tools are unchanged. Still nothing run against live GitLab. | **Rename `LXAGENTS_MCP_GITLAB_API_KEY` to `MCP_AUTH_TOKEN`** in the `env` block of the server's entry (and in `docker run -e`), and set it only there, never exported in a shell, since `shared-instruction` uses the same name for a different secret. To use HTTP, register the URL with an `Authorization: Bearer` header and run `npm run start:http`. Restart the client. |
| [`0/1/0`](../../wiki/logs/0/1/0/CHANGELOG.md) | 2026-10-10 | **The first release.** A stdio MCP server for GitLab in 45 tools: projects, branches, files, commits, tags, releases, issues, merge requests and CI/CD pipelines. `repo_create` is `create_gitlab_repo` moved from `cli` 2.1.0 and renamed, otherwise unchanged; the token is now sent as `Authorization: Bearer`. Nothing has run against live GitLab, and the image has never been built. | **Create a token and register the server** with `LXAGENTS_MCP_GITLAB_API_KEY` in its `env` block (clone, `npm install`, `node src/index.js`); restart the client; run the smoke test before relying on a write tool. **If you called `create_gitlab_repo` on `cli`, call `repo_create` here** — the variable has the same name but belongs in this server's `env` block. |

## Maintenance

* Newest version first, one row per version directory.
* A new version directory is a version claim and needs the owner's approval.
* Never edit a released log to change history; corrections go in the next version.
