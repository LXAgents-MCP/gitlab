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
| [`0/1/0`](../../wiki/logs/0/1/0/CHANGELOG.md) | 2026-10-10 | **The first release.** A stdio MCP server for GitLab in 45 tools: projects, branches, files, commits, tags, releases, issues, merge requests and CI/CD pipelines. `repo_create` is `create_gitlab_repo` moved from `cli` 2.1.0 and renamed, otherwise unchanged; the token is now sent as `Authorization: Bearer`. Nothing has run against live GitLab, and the image has never been built. | **Create a token and register the server** with `LXAGENTS_MCP_GITLAB_API_KEY` in its `env` block (clone, `npm install`, `node src/index.js`); restart the client; run the smoke test before relying on a write tool. **If you called `create_gitlab_repo` on `cli`, call `repo_create` here** — the variable has the same name but belongs in this server's `env` block. |

## Maintenance

* Newest version first, one row per version directory.
* A new version directory is a version claim and needs the owner's approval.
* Never edit a released log to change history; corrections go in the next version.
