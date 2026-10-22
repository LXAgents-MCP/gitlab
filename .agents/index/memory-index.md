---
name: memory-index
description: Index of this repository's memory — current state and task records. Read every session.
---

# Memory Index

**Scope:** `.agents/memory/`
**Parent:** [`root-index.md`](root-index.md)

Read this every session and load only the rows whose scope matches the request, so you
continue prior work instead of restarting it. Every memory file is registered here in the
same commit that creates it.

## state/

| File | Purpose |
|---|---|
| [`../memory/state/repository-state.md`](../memory/state/repository-state.md) | Current known state: what exists, what is verified, what is not built yet. |

## decisions/

| File | Purpose |
|---|---|
| [`../memory/decisions/stdio-only-transport.md`](../memory/decisions/stdio-only-transport.md) | Superseded: why this server was stdio only. |
| [`../memory/decisions/http-token-transport.md`](../memory/decisions/http-token-transport.md) | Partly superseded: why this server also serves HTTP, and the passthrough design of 0.2.0. |
| [`../memory/decisions/two-token-auth.md`](../memory/decisions/two-token-auth.md) | Why this server holds two secrets, `MCP_AUTH_TOKEN` for the client and `GITLAB_API_KEY` for GitLab, and what that costs compared with passing the client's token through. |
| [`../memory/decisions/http-bind-defaults.md`](../memory/decisions/http-bind-defaults.md) | Why the HTTP transport defaults to port 3000 on all interfaces, as `shared-instruction` does, what guards a server that listens everywhere by default, and the one place its `PORT` parsing is not copied. |

## tasks/

| File | Purpose |
|---|---|
| [`../memory/tasks/forge-servers.md`](../memory/tasks/forge-servers.md) | Building this server: `repo_create` moved from `cli`, then the 45-tool GitLab surface; the `0.1.0` release. |
| [`../memory/tasks/http-token-auth.md`](../memory/tasks/http-token-auth.md) | Giving this server an HTTP transport with a required bearer token and the one variable `MCP_AUTH_TOKEN`; released as 0.2.0. |
| [`../memory/tasks/two-token-auth.md`](../memory/tasks/two-token-auth.md) | Separating the token a client presents to this server (`MCP_AUTH_TOKEN`) from the key the server uses against GitLab (`GITLAB_API_KEY`); released as 0.3.0. |
| [`../memory/tasks/http-defaults.md`](../memory/tasks/http-defaults.md) | Giving the HTTP transport the same defaults as `shared-instruction`, port 3000 on all interfaces; released as 0.4.0. |
| [`../memory/tasks/src-architecture.md`](../memory/tasks/src-architecture.md) | Splitting the flat `src/` into `routes/`, `models/`, `services/`, `controllers/` (was `src/tools/`), `utils/` and `config/`, alongside `github`; released as 0.5.0. |
