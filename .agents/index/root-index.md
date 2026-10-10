---
name: root-index
description: Router for LXAgents-MCP/gitlab — every local index, the shared set, and the override table.
---

# Root Index

This file lists indexes only. Never rules, never documentation, never leaf content.
Read exactly one branch per task, plus `memory-index.md`.

## Indexes

| Index | Scope | Load when |
|---|---|---|
| [`agents-index.md`](agents-index.md) | `.agents/rules/` — this repository's own rules | You need a rule specific to this repository, including anything about the tool surface. |
| [`agent-wiki-index.md`](agent-wiki-index.md) | `.agents/wiki/` agent knowledge | You need orientation before touching code, or the security procedure. |
| [`project-wiki-index.md`](project-wiki-index.md) | `wiki/` human documentation | You need to read or write documentation a person will read. |
| [`logs-index.md`](logs-index.md) | `wiki/logs/` release history | You need what changed in a version, or what clients must do about it. |
| [`memory-index.md`](memory-index.md) | `.agents/memory/` dynamic state | You need prior task state, or must record progress. |

The shared set is not a local file and is not indexed here: the `automation` tool of the
connector lists every shared tool and the condition that activates it.

## Shared overrides

| `name` | Local file | Replaces | Why |
|---|---|---|---|

No overrides — this repository uses the shared set unchanged. `gitlab-repository-rules` and
`tool-surface` are additions, not overrides: neither `name` exists in the shared set.

## Maintenance

* Adding, removing, or renaming any index updates this table **in the same commit**.
* Adding or dropping an override updates the override table in the same commit.
