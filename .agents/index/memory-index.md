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
| [`../memory/decisions/stdio-only-transport.md`](../memory/decisions/stdio-only-transport.md) | Why this server is stdio only, when a sibling repository serves HTTP. |

## tasks/

| File | Purpose |
|---|---|
| [`../memory/tasks/forge-servers.md`](../memory/tasks/forge-servers.md) | Building this server: `repo_create` moved from `cli`, then the 45-tool GitLab surface; the `0.1.0` release. |
| [`../memory/tasks/http-token-auth.md`](../memory/tasks/http-token-auth.md) | Giving this server an HTTP transport with a required bearer token and the one variable `MCP_AUTH_TOKEN`; released as 0.2.0. |
