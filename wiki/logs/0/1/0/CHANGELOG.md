# 0.1.0

**Released:** 2026-10-10

The first release of `lxagents-gitlab`, an MCP server that gives an LLM access to GitLab through
the REST API: projects, branches, files, commits, tags, releases, issues, merge requests and CI/CD
pipelines, in 45 tools. `repo_create` is the `create_gitlab_repo` tool of `cli` 2.1.0, moved here
and renamed, with the same arguments and behaviour; `cli` drops its copy in its own 3.0.0.

**Nothing has run against live GitLab.** The environment this was built in had no GitLab token, so
every write request is checked against GitLab's documentation and against mocks, and no more; the
reads were also checked against public projects on `gitlab.com` where that was possible. The write
tools — everything that creates, changes, merges or deletes — are the ones to prove first.
[Smoke test](../../../../guides/smoke-test.md) is the walk, and lists the behaviours the
documentation leaves open.

**The server is stdio only, with no network listener.** It makes outbound HTTPS requests, to
`gitlab.com` only. It does not support self-managed GitLab instances.

**Clients must, to use it:**

1. **Create a token.** A personal access token with the `api` scope, or a project or group access
   token where the tools you will use allow it; creating a project needs a personal one. What each
   tool needs is in [Environment](../../../../environments/env.md#what-the-token-needs).
2. **Register the server** in the client configuration with the token in its `env` block, as
   `LXAGENTS_MCP_GITLAB_API_KEY`: clone the repository, run `npm install` once, and point the client
   at `node src/index.js`. See [Connect a client](../../../../guides/connect-a-client.md). The package
   is private and is not published to npm.
3. **If you called `create_gitlab_repo` on the `cli` server, call `repo_create` on this one.** The
   variable has the same name but must now be in this server's `env` block, not `cli`'s. Until `cli`
   3.0.0 lands both exist.
4. **Restart the client**, so it reloads the connector and lists the tools.
5. **Run the smoke test** against a project you can throw away before relying on a write tool.

A server without the variable still starts; a tool then refuses with a message naming it.

## Added

- **Projects:** `repo_create`, `repo_read`, `repo_update`, `repo_delete` (which refuses unless
  `confirm` repeats the project's full path; on gitlab.com GitLab schedules the deletion and keeps
  the project for a retention period), `repo_fork` and `repo_search`.
- **Branches:** `branch_create`, `branch_read`, `branch_list`, `branch_delete` and `branch_merge`.
  GitLab has no direct merge, so `branch_merge` opens a merge request from the head branch into the
  base and accepts it at once; if it cannot be merged the request it opened is closed again.
- **Files and directories:** `file_read` (cut at 25,000 characters; binary files and files over 1 MB
  refused), `directory_list`, `file_create`, `file_update` and `file_delete`. Each write is one
  commit on a branch you name, and a stale `last_commit_id` is refused.
- **Commits:** `commit_read` (optionally with its diff, cut at 20,000 characters), `commit_list`
  and `commit_compare`.
- **Tags and releases:** `tag_check` (a missing tag is an answer, not an error), `tag_get`,
  `tag_get_all`, `tag_delete`, `release_create` (which makes the tag when it does not exist),
  `release_read`, `release_list` and `release_delete`. There is no separate tag-creating tool. A
  release has `released_at` where GitHub's has draft and prerelease: GitLab has neither.
- **Issues:** `issue_open`, `issue_read`, `issue_comment`, `issue_close`, `issue_assign`,
  `issue_label` and `issue_lock`, addressed by the number in the project. Usernames are looked up
  first and an unknown one refuses the call. GitLab creates a label that does not exist, and a free
  plan keeps one assignee, so the tools report what was applied and name what was not.
- **Merge requests:** `pr_open` (from a branch of the same project, optionally as a draft), `pr_read`
  (with the changed files and the comments on request), `pr_comment`, `pr_close`, `pr_merge` (with
  `squash`; how a merge lands is the project's setting), `pr_review_request` and `pr_approve`.
  `pr_read` says why a request cannot be merged yet, and that GitLab is still working it out
  instead of answering no, and `pr_merge` asks GitLab why when it refuses.
- **Pipelines:** `workflow_list`, `workflow_run` (on a branch or tag, with variables),
  `workflow_status` (a pipeline by id, or the latest on a branch or tag, with its jobs and the
  reason a failed one failed) and `workflow_logs` (the failed jobs' logs by default, cut to their
  ends, 20,000 characters in all, with colour codes and section markers removed).
- **One way to name things.** Every tool but `repo_create` and `repo_search` takes a single `repo`
  argument, the project's full path such as `group/project` or its numeric id. Lists take `page` and
  `per_page` and say whether another page follows. Each result is readable text with the same fields
  as structured content, and long content is cut at a stated limit and says so.
- **Honest annotations.** Every tool is marked as reading, creating, changing or destroying, and a
  test holds it to that, so a client can ask a person before anything destructive.
- **Failures that say what to do next:** a rejected token, a missing permission with GitLab's own
  hint, a rate limit with its reset time, a name already taken, a merge request that cannot be merged
  and why.
- **`--help` and `--version`**, which write to stderr and leave stdout to the protocol.
- **A `Dockerfile`** for a stdio image, which has never been built. See
  [Docker](../../../../environments/docker.md).
- **The server closes and exits on `SIGTERM` as well as `SIGINT`**, which the image needs.
- **Documentation:** the README, the wiki, and the repository's own agent instruction set, which
  consumes the shared set through the connector and copies none of it.
- **Tests:** 272 under `node --test`. `fetch` is replaced and an unexpected request fails the test; the
  one exception, a redirect check, talks to two servers on loopback only.

## Changed

- **`repo_create` is `create_gitlab_repo` from `cli`**, renamed. Its arguments, defaults, results and
  refusal wording are unchanged.
- **The token is sent as `Authorization: Bearer`**, where `cli` sent `PRIVATE-TOKEN`. GitLab accepts a
  personal, project or group access token either way, and nothing a caller sees changes; the runtime
  drops `Authorization` when a redirect changes host, and would have forwarded the other header.

## Security

- **The token is read from the environment at call time.** It is never an argument, so it does not
  appear in a prompt or a transcript, and a server without one still starts.
- **The token is never logged or returned.** It is removed from every message the server can produce,
  refusals and crashes alike, and a test makes GitLab, the network and a handler echo it.
- **One place builds URLs.** A project, a branch or tag name and a file path are validated and
  percent-encoded there, so an argument cannot steer a call at another endpoint, and no argument can
  choose the host.
- **A redirect to another host does not carry the token.** GitLab moves a renamed project by
  redirecting, and the runtime drops the `Authorization` header when that changes host; a test shows a
  second host receiving no token.
- **The token's permissions are the real limit.** GitLab enforces them, not this server: give it only
  what the tools in use need.
