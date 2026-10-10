# MCP Surface

Everything `lxagents-gitlab` exposes.

**It exposes tools, and nothing else.** One per module in `src/tools/`. There are no prompts and no
resources, and one transport: stdio. See [What this server does not expose](#what-this-server-does-not-expose).

## Server identity

| Field | Value |
|---|---|
| `name` | `lxagents-gitlab` |
| `version` | from `package.json`, read at import by `src/version.js` |
| transport | stdio (`src/index.js`) |

`initialize` also returns `instructions`: that every tool needs `LXAGENTS_MCP_GITLAB_API_KEY`, that
a project is named by its full path or numeric id, and that tools which delete or merge are
destructive and should be confirmed with the owner first.

## Conventions every tool shares

### Naming

A tool is `{thing}_{action}`: `repo_create`, `branch_list`, `pr_merge`. The module that defines it
is the same name in kebab case, `src/tools/pr-merge.js`. The names are the GitHub server's: here
`repo_` acts on a project, `pr_` on a merge request and `workflow_` on a pipeline.

### Addressing a repository

Every tool that acts on a repository takes one `repo` argument: the project's full path, such as
`group/project` or `group/subgroup/project`, or its numeric id. The two that cannot are
`repo_create`, which has no project yet, and `repo_search`, which looks across all of them. `repo` is
parsed once, in `src/api.js`, and a value that is neither a path of valid names nor a number is
refused before any request. A path is sent as the single encoded segment GitLab expects.

### Results

Each result is readable text plus `structuredContent` that matches the tool's output schema. The
text opens with a one-sentence summary, then lists the fields; long content — a file, a log, a diff
— is a fenced block cut at a stated limit, with `truncated: true` when it was cut.

### Paging

A list takes `page` (from 1) and `per_page` (1 to 100, default 30) and returns `page`, `per_page`
and `has_more`. Ask for the next page while `has_more` is true.

### Annotations

| Kind | Meaning | Hints |
|---|---|---|
| `read` | Only reads | read-only, idempotent |
| `write` | Creates something | none of those |
| `update` | Changes something that exists | idempotent |
| `destroy` | Deletes, overwrites or merges | destructive, idempotent |

All four set `openWorldHint`, because every tool talks to GitLab. `test/surface.js` pins every
tool's kind, and a test holds the annotations to it.

### Failures

A failure is an `isError` result whose text says what happened and what to do: a rejected token
(401), a missing permission (403), a rate limit with its reset time, a project the token cannot
see (404), a conflict (409), a validation error with GitLab's own reasons (400 or 422). The token
never appears in one.

## The tools

### Projects

| Tool | Kind | Does |
|---|---|---|
| `repo_create` | write | Create a project, in the token owner's namespace or in a group. Private unless told otherwise, with an initial README. Moved from `cli`, where it was `create_gitlab_repo`. |

## What this server does not expose

| Not exposed | Why |
|---|---|
| Prompts and resources | Everything an agent needs is a call with arguments. |
| Local files and commands | That is `cli`'s job; this server touches GitLab only. |
| HTTP | The token can delete projects. See [Security model](../security/security-model.md). |
| Self-managed GitLab | The API base is a constant, and another host would need a second setting. |
| `clone`, `push` and other git transport | File tools commit through the API instead. |
| `issue_list`, `pr_list`, `tag_create`, `issue_reopen`, inline review comments | Outside the tool list this server was built to. A tag is created by `release_create`. |
