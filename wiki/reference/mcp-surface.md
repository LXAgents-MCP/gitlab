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
| `repo_read` | read | Read one project: description, visibility, default branch, star, fork and open issue counts, license, topics, last activity and parent, and the date it is scheduled for deletion, if it is. |
| `repo_update` | update | Change settings: name, URL path, description, visibility, default branch, archive or unarchive, the issue tracker switch. Only what is passed changes; at least one is required. Archiving has its own endpoint in GitLab, so one call may make up to two requests. |
| `repo_delete` | destroy | Delete a project. `confirm` must repeat its full path, and anything else deletes nothing. GitLab may only mark the project for deletion and remove it after a retention period, so "accepted" is not "gone": it can usually be restored until then. |
| `repo_fork` | write | Fork into the token owner's namespace or a group. GitLab copies it in the background, so the fork may take a moment to be readable; its `import_status` says when it is done. |
| `repo_search` | read | Search projects by words in their name, path or description; every word must match. Paged, with GitLab's total when it gives one. It sees the projects the token's user can see. |

### Branches

A branch name with a slash is written as one encoded path segment, `feat%2Fx`, which is the only form
GitLab finds: the same name with a literal slash is a 404, as a request against `gitlab.com` showed.

| Tool | Kind | Does |
|---|---|---|
| `branch_create` | write | Create a branch from the default branch, another branch, a tag or a sha. Fails if the name is taken. |
| `branch_read` | read | Read one branch: the commit at its tip, that commit's title and date, and whether the branch is protected or the default. |
| `branch_list` | read | List branches with the sha at each tip and whether each is protected, the default or merged. Paged; can search by name, with `^feat` for a start and `fix$` for an end. |
| `branch_delete` | destroy | Delete a branch. The default branch and protected branches cannot be deleted. |
| `branch_merge` | destroy | Merge one branch into another. GitLab has no direct merge, so this opens a merge request from head into base and accepts it at once; the merged request stays as the record, and a request that cannot be merged is closed again. Waits up to ten seconds for GitLab to work out whether it can. Moves the base branch. |

### Files and directories

Changes are made by committing through the API, one commit per call, on a branch you name (default:
the default branch). A path is written as one encoded segment, `src%2Findex.js`. GitLab takes a
commit's content as text or base64, and this server always sends base64, so the text arrives exactly as
it was given.

| Tool | Kind | Does |
|---|---|---|
| `file_read` | read | Read one text file at a branch, tag or commit; `HEAD`, the default, is the default branch. Cut at 25,000 characters; binary files and files over 1 MB are refused. Returns its content, size, blob id and `last_commit_id`. |
| `directory_list` | read | List the files and folders in one directory, not recursively. Paged. Omit the path for the root. |
| `file_create` | write | Create a new file as one commit. Fails if it exists. Content up to 1 MB. |
| `file_update` | update | Replace an existing file's whole content as one commit. Pass `last_commit_id` from `file_read` to fail if the file changed since. |
| `file_delete` | destroy | Delete one file as one commit. The file stays in history. |

GitLab's files API does not return the commit it made, only the path and branch, so a write does not
report a commit sha; `commit_list` finds it.

### Commits

| Tool | Kind | Does |
|---|---|---|
| `commit_read` | read | Read one commit: message, author, date, parents, and the files it changed with line counts. `diff: true` adds its diff, cut at 20,000 characters. Defaults to the default branch's tip. Makes two requests, because GitLab keeps a commit's files in its diff. |
| `commit_list` | read | List commits, newest first, from a branch, tag or sha; optionally only those touching a path, by an author, or within a date range. Paged. |
| `commit_compare` | read | Compare two refs: ahead and behind, the most recent 50 commits between them, and the changed files (the first 100) with line counts. GitLab compares in one direction, so ahead and behind are two requests. |

GitLab sends a file's diff without counts, so the line counts are read off its hunks.

### Tags and releases

A tag is created by `release_create`, which makes the tag from a branch or commit when it does not
exist; there is no separate tag-creating tool. A release is named by its tag: GitLab gives it no id of
its own. GitLab has no draft or prerelease flag and does not write release notes, so `release_create`
has none of those; it has `released_at`, and a date in the future makes an upcoming release.

| Tool | Kind | Does |
|---|---|---|
| `tag_check` | read | Say whether a tag exists, and if so what it points at and whether it is annotated. A missing tag is an answer, not an error; a missing project is an error. |
| `tag_get` | read | Read one tag: the commit behind it, whether it is protected and has a release, and, for an annotated tag, its message and date. |
| `tag_get_all` | read | List tags, most recently updated first, with the commit each points at. Paged; can search by name. |
| `tag_delete` | destroy | Delete a tag. It does not delete a release. A protected tag cannot be deleted. |
| `release_create` | write | Create a release for a tag, creating the tag from the default branch or a target if it does not exist. Optional title, notes and release date. |
| `release_read` | read | Read one release by tag, or the latest through GitLab's permanent address: title, notes, date, asset links and source archives. |
| `release_list` | read | List releases, newest release date first, upcoming ones included. Paged. |
| `release_delete` | destroy | Delete a release by tag. Its tag is kept. |

### Issues

An issue is named by its number in the project, the `#42` people write, which GitLab calls the `iid`.
It is not the global id in the API's answers. Merge requests are numbered separately, so `#7` the
issue and `!7` the merge request can both exist; these tools only ever address the issue. Usernames
are looked up before anything is written, and an unknown one refuses the whole call, so a typo never
assigns half of what was asked.

| Tool | Kind | Does |
|---|---|---|
| `issue_open` | write | Open an issue, with optional body, labels, assignees and milestone. Reports what was applied and what was not. |
| `issue_read` | read | Read one issue: state, author, labels, assignees, milestone, body. `comments: true` adds the first 30 comments people wrote, without GitLab's system notes. |
| `issue_comment` | write | Add a comment. On a locked issue only project members can. |
| `issue_close` | update | Close an issue, optionally leaving a comment first. GitLab has no close reason, and there is no tool to reopen. |
| `issue_assign` | update | Add, remove or replace (`set`) the assignees, by username. Reports any person GitLab did not apply. |
| `issue_label` | update | Add, remove or replace (`set`) the labels. Reports any label not as asked. |
| `issue_lock` | update | Lock or unlock the discussion. GitLab takes no reason. |

GitLab differs from GitHub here in four ways that show in these tools. A label that does not exist is
created, so a misspelled label becomes a new one, and a name with a comma in it cannot be sent and is
refused. A free plan keeps one assignee per issue, which is why the result lists who was not applied. A
milestone is given by its id, not its title. And a comment's answer carries no address, so the link
returned is built from the project path, and is left out for a project given by id.

### Merge requests

A merge request is named by its number in the project, the `!7` people write, which GitLab calls the
`iid`. It is numbered apart from issues, so `issue_read` cannot read a merge request: `pr_read` takes
`comments: true` for its discussion. `pr_open` takes a branch of the same project; a branch of a fork is
not supported. A draft is a title that starts with `Draft:`, which `pr_open` adds.

| Tool | Kind | Does |
|---|---|---|
| `pr_open` | write | Open a merge request from a branch into a base (default: the default branch), optionally as a draft. |
| `pr_read` | read | Read one merge request: state, draft, whether it can be merged and if not why, branches, labels, reviewers, description. `files: true` adds the first 100 changed files with line counts; `comments: true` the first 30 comments. |
| `pr_comment` | write | Add a general comment to the discussion. |
| `pr_close` | update | Close a merge request without merging, optionally leaving a comment first. There is no tool to reopen it. |
| `pr_merge` | destroy | Merge a merge request, optionally squashing, and optionally only if the branch is still at a given commit. |
| `pr_review_request` | update | Add reviewers, by username, to those already asked. Reports any person GitLab did not add. |
| `pr_approve` | update | Approve as the token's user, optionally only for a given commit. Reports who has approved and how many approvals are left. |

GitLab differs from GitHub here in four ways that show in these tools. How a merge lands (a merge
commit, a fast-forward or a rebase) is a setting of the project, so `pr_merge` has `squash` and no
`method`. An approval carries no message and a reviewer cannot be a team, so `pr_approve` has no `body`
and `pr_review_request` no `team_reviewers`. Whether a request can be merged is the single verdict
GitLab computes from conflicts, pipelines, approvals and drafts, in the background, so `pr_read` leaves
`mergeable` out while that is running and gives the reason when it is false. And what happened to a
request (a push, a label) is recorded in the same list as its comments, as system notes, which
`pr_read` and `issue_read` leave out.

### Pipelines

A project has one pipeline definition, its `.gitlab-ci.yml`, so there is no list of workflows to pick
from: these tools act on pipelines, the runs. A pipeline is named by its id, from `workflow_run` or
`workflow_list`, and a job by its id, from `workflow_status`.

| Tool | Kind | Does |
|---|---|---|
| `workflow_list` | read | List recent pipelines, newest first, optionally of one branch or tag or in one status. Paged. |
| `workflow_run` | write | Start a pipeline on a branch or tag (default: the default branch), with optional CI/CD variables. It runs whatever the pipeline does, which may deploy or publish. |
| `workflow_status` | read | Read a pipeline by id, or the latest one on a branch or tag: status, commit, timing, and every job with the reason a failed one failed. A ref with no pipeline yet is an answer, not an error. |
| `workflow_logs` | read | Read job logs: by default the failed jobs, up to five, each cut to its end, within 20,000 characters in all. `job_id` reads one job; `only_failed: false` reads every job. |

GitLab differs from GitHub here in three ways that show in these tools. There is no workflow to name,
so `workflow_run` takes a `ref` and `variables` where GitHub takes a workflow, a ref and `inputs`. A
job has no steps, so a failed job carries GitLab's `failure_reason` (`script_failure`,
`runner_system_failure`) and its log says where. And a runner's log carries colour codes and
`section_start` markers that fold it in the web page, which `workflow_logs` strips. Setting variables
may be limited to a higher role by the project, and a variable is shown in the pipeline, so it is never
a place for a secret.

## What this server does not expose

| Not exposed | Why |
|---|---|
| Prompts and resources | Everything an agent needs is a call with arguments. |
| Local files and commands | That is `cli`'s job; this server touches GitLab only. |
| HTTP | The token can delete projects. See [Security model](../security/security-model.md). |
| Self-managed GitLab | The API base is a constant, and another host would need a second setting. |
| Rebasing a merge request's branch, and merging when its pipeline succeeds | GitLab has endpoints for both; neither is among the tools this server was built to. |
| `clone`, `push` and other git transport | File tools commit through the API instead. |
| `issue_list`, `pr_list`, `tag_create`, `issue_reopen`, inline review comments | Outside the tool list this server was built to. A tag is created by `release_create`. |
