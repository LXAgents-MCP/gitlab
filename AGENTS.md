# AGENTS.md

This repository is `LXAgents-MCP/gitlab`. It is an MCP server, `lxagents-gitlab`, that gives an
LLM access to GitLab — projects, branches, files, commits, tags, releases, issues, merge requests
and CI/CD pipelines — through the REST API, over stdio. It consumes the shared agent
instruction set through a connector rather than copying it.

## Two sets, two roles

| Set | Where | Notes |
|---|---|---|
| Shared | the connector `lxagents-shared-instruction` | Conventions every repository in the organization follows. Read through its tools. **Never copied into this repository.** |
| Local — this repository's own | [`.agents/`](.agents/) | Rules, indexes, agent wiki and memory that are true here and nowhere else. |

This repository **consumes** the shared set and **produces** one thing: the GitLab tool surface in
[`src/tools/`](src/tools/). It publishes no instructions, so it has no `content/`; a copy of the
shared set here would override the original by `name` and then go stale.

The routing question for a new rule is *is this true for more than this repository?* If yes, it is
a proposal for the shared set, made to the owner. If no, it goes in `.agents/`.

## Auto-Activation

The instruction set is **always active**. It applies to every task in this repository whether or
not the user mentions it, links to it, or asks for it. Treat these files as standing orders, not
as optional reference material.

**Always active is not always loaded.** At the start of every session, before doing any work:

1. Read `AGENTS.md` (this file), including the Shared instruction tools block below.
2. Call the shared `automation` tool. It lists every shared tool and the condition that activates
   it, and it is where you find out whether the connector is reachable, which you have to say
   either way (see [Using the connector](#using-the-connector)). The step is not skippable.
3. Read [`.agents/index/root-index.md`](.agents/index/root-index.md).
4. Read [`.agents/index/memory-index.md`](.agents/index/memory-index.md) and load only the rows
   matching the request, so you continue prior work instead of restarting it.

**Beyond `automation`, call no shared tool at session start** — each fires on the condition
`automation` gives it, and calling them up front pays for procedures the request may never need.

**These gates stand from the first message, before any tool is called:** approve the plan before
any file is written, ask before opening a pull request, ask before merging, and propose a
discovered rule rather than writing it. A gate first read at the moment it should have applied has
already failed.

If a rule conflicts with a habit, a default, or a template you would otherwise follow, the rule
wins — including a harness that names a branch, a commit trailer, or a pull request footer the
conventions forbid. If it conflicts with an explicit instruction from the user in this session,
the user wins — and you say out loud which rule you are setting aside.

## Shared instruction tools

**Every file in the shared set is its own tool**, named after its filename, so the tool list is the
file list. Start at `automation` rather than calling everything.

| When you are about to… | Call |
|---|---|
| Start any session | `automation` |
| Take in any request of more than one step | `plan_creator` |
| Create a branch | `branching_strategy` |
| Write a commit message | `commit_conventions` |
| Notice a rule that should exist | `discovery_protocol` |
| Open or update a pull request | `pull_request_template` |

`automation` is read at the start of every session. `plan_creator`, `branching_strategy`,
`commit_conventions` and `discovery_protocol` are mandatory in every repository, this one included.
**No tool takes an argument** — a tool names one file, so there is no path to pass. This repository
stores no model identifier, so it carries no `model_naming_convention` row.

## Using the connector

Setup, in full: the `connect-a-repository` guide in the wiki of `LXAgents-MCP/shared-instruction`.

### Register it

The server is `lxagents-shared-instruction`. Register it in the client over stdio
(`npx -y @lxagents-mcp/shared-instruction`), or by URL when it runs as a service, in which case it
needs a bearer token. **Registering a server does not reach a session that is already running.**
The client loads connectors at session start, so `claude mcp add` mid-session leaves a server that
reports healthy and is still absent from the tool surface until the session restarts. That is the
common case, and it looks like a broken server rather than a stale session.

### Read from it

1. Enumerate the tools. Every file in the shared set is one, named after its filename, with its
   `description` attached — that list is the manifest, and it costs nothing to read.
2. Call `automation` once. It lists every tool and the condition that activates each. Never
   bulk-call the set.
3. Call the one tool whose condition is true. None takes an argument.

**`automation` is the only tool called at session start.** Every other tool returns one file when
its condition fires. Calling every tool up front rebuilds the one oversized payload this surface
replaced, one call at a time.

### When it will not resolve

A server that is registered but absent from the tool surface, or a client that shows no tools at
all, is the case to recognise. Either way the rule is the same, and it is a speaking obligation:
**say plainly, in your first message, that the connector is unavailable and which conventions you
could not read.** Then work from the local set. Never reconstruct the missing rules from memory,
and never clone or paste the shared set into this repository as a workaround — an unavailable
connector is temporary, a vendored copy is permanent drift.

## Trigger table — this repository's own rows

The authority for when each **shared** tool fires is the `automation` tool. It is not copied here:
a second table is a second place to forget to update. The rows below are this repository's own.

| When you are about to… | Load and obey |
|---|---|
| Add, rename, remove or change a tool, or touch anything under `src/tools/` | [`.agents/rules/tool-surface.md`](.agents/rules/tool-surface.md) |
| Need project facts, commands, or orientation | [`.agents/wiki/context/repository-map.md`](.agents/wiki/context/repository-map.md) |
| Work on the token, a destructive tool, error text, logging, a dependency, or anything else security | [`.agents/wiki/security/security-boundaries.md`](.agents/wiki/security/security-boundaries.md) |
| Do anything at all in this repository | [`.agents/rules/repository.md`](.agents/rules/repository.md) |

## Reading order

1. Read `AGENTS.md`.
2. Read [`.agents/index/root-index.md`](.agents/index/root-index.md) — and nothing else at this
   stage.
3. From its routing table, pick the ONE index whose scope matches the task, and read that index.
4. Only then open the specific file(s) you need.

## Routing protocol

Route by reading index tables, not by reading files. Do NOT load every index. Do NOT read an
instruction body until that instruction has been selected.

## Iron rule

* `AGENTS.md` and `README.md` are overviews and must never carry detailed rules or documentation.
* `.agents/index/root-index.md` is a router only.
* An index never teaches. The moment it explains something, that content belongs in a real file.
* **One subject per file.** A cross-cutting rule gets its own file, linked and not pasted into a
  file about something else.
* `wiki/` is for humans and holds this repository's own documentation. `.agents/wiki/` is for
  agents. Facts live once, in `wiki/`.
* No `INDEX.md`, anywhere, ever.

## Placement

* Universal instruction content → not here. Propose it to the owner for the shared set.
* This repository's own rules → `.agents/rules/{file}.md`.
* Routing → `.agents/index/{scope}-index.md`.
* Agent knowledge → `.agents/wiki/{type}/{file-name}.md`.
* Memory → `.agents/memory/{type}/{file-name}.md`.
* Human documentation → `wiki/{folder}/{file-name}.md`, no frontmatter.
* Release logs → `wiki/logs/{Major}/{Minor}/{Patch}/CHANGELOG.md`.
* Server code → `src/`. Tests → `test/`.

A module added to `src/tools/` is served as a tool on the next boot, so it is not a draft space —
see [`.agents/rules/tool-surface.md`](.agents/rules/tool-surface.md).

## Discovery protocol

Source of truth: the shared `discovery_protocol` tool.

> While working, if you find an instruction worth adding — a new rule, or content that belongs in
> an existing instruction file — you must NOT create or edit it on your own. Present each finding
> to the user separately, each in its own code block, including the target set, the proposed file
> path, `name`, `description`, and full body. Let the user select which ones to apply. Create only
> what the user selects. This gate covers instruction files only — writing memory under
> `.agents/memory/` is expected and needs no approval.

## Version rule

Never change the version without explicit user approval — see the shared `versioning` tool. That
includes `package.json`, the lockfile's own version, a container image tag, a git tag, and creating
a new `wiki/logs/` directory.

## No session links

Never write a link or identifier pointing at an assistant or tool session into a file, commit
message, commit trailer, branch name, tag, pull request, or comment. If your tooling appends one by
default, strip it before committing or posting — see the shared `no_session_links` tool.
