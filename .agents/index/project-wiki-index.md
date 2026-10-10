---
name: project-wiki-index
description: Index of the human documentation in wiki/ — what this server is, how it is built, and how to run and connect it.
---

# Project Wiki Index

**Scope:** `wiki/` (except `logs/`)
**Parent:** [`root-index.md`](root-index.md)

Plain markdown, no frontmatter, written for people. Any page added or removed is reflected here in
the same commit.

## information/

| File | Purpose |
|---|---|
| [`../../wiki/information/overview.md`](../../wiki/information/overview.md) | What the server delivers, why it is its own server, and what it is not. |
| [`../../wiki/information/architecture.md`](../../wiki/information/architecture.md) | The discovered registry, a call from argument to result, and shutdown. |

## reference/

| File | Purpose |
|---|---|
| [`../../wiki/reference/mcp-surface.md`](../../wiki/reference/mcp-surface.md) | Every tool and the conventions they share, and what the server does not expose. |

## environments/

| File | Purpose |
|---|---|
| [`../../wiki/environments/setup.md`](../../wiki/environments/setup.md) | Installing, running, inspecting and testing it. |
| [`../../wiki/environments/env.md`](../../wiki/environments/env.md) | The one variable, and what the token needs. |
| [`../../wiki/environments/docker.md`](../../wiki/environments/docker.md) | Building and running the stdio image, registering it with a client, and what it has not been checked for. |

## guides/

| File | Purpose |
|---|---|
| [`../../wiki/guides/connect-a-client.md`](../../wiki/guides/connect-a-client.md) | Registering the server with Claude Desktop or Claude Code, with a token. |

## security/

| File | Purpose |
|---|---|
| [`../../wiki/security/security-model.md`](../../wiki/security/security-model.md) | The token, the surfaces that touch it, what to grant, and what is deliberately not protected. |
