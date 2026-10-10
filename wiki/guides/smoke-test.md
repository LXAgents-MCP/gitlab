# Smoke Test

**Nothing in this repository has run against live GitLab with a token.** Every write request is checked
against GitLab's documentation and against mocks, which prove that the server sends what it means to
send, not that GitLab accepts it; several reads were also checked against public projects on
`gitlab.com`. This page is the first live run: a walk through every tool on a project you can throw
away, then the behaviours the documentation leaves open, each with what to look for.

Do it once after installing, and again after a change to a request. **Use a throwaway project and a
token you can revoke.**

## Before you start

* **A personal access token** with the `api` scope. Creating, forking and deleting a project need a
  user's own token; a project or group access token cannot create one. The rest of the walk also works
  with a project access token for the scratch project, with what
  [Environment](../environments/env.md#what-the-token-needs) lists.
* **The server registered, and the client restarted**, so that it lists the tools. See
  [Connect a client](connect-a-client.md).
* **Something small and public to fork**, such as `gitlab-org/gitlab-test`.
* **An account that can run pipelines.** On gitlab.com a new account may have to be verified before
  its pipelines run on shared runners; a pipeline that fails to start then says so in its
  `failure_reason`, and rows 14 and 15 show it instead of a failing job.

## The walk

The project is `scratch-delete-me`; replace `<you>` with the token owner's username. Each row is one
call, and later rows use what earlier ones made.

| # | Tools and arguments | Expect |
|---|---|---|
| 1 | `repo_create` `name: scratch-delete-me`, `visibility: private`, `initialize_with_readme: true` | A private project with a README and the default branch `main`. |
| 2 | `repo_read`, then `repo_update` `description: scratch`, then `repo_read` | The description is set; nothing else changed. |
| 3 | `repo_search` `query: scratch-delete-me` | The project — or no match for a minute, while GitLab's search catches up. |
| 4 | `file_create` `path: docs/a.md`, `content: one`, `message: add a`; `file_read`; `directory_list` `path: docs` | A commit; the same text and a `last_commit_id` back; `a.md` listed. |
| 5 | `file_create` the same path again | Refused, saying the file exists. |
| 6 | `file_update` with the `last_commit_id` from `file_read`; then again with the old one | The first commits; the second is refused as stale ("changed since"). |
| 7 | `branch_create` `branch: feat/smoke`; `branch_read`; `branch_list` | A branch from `main` — the slash in its name is the point — with the same tip as `main`. |
| 8 | `file_create` `path: docs/b.md`, `branch: feat/smoke` | The file is on `feat/smoke` and not on `main`. |
| 9 | `commit_list` `ref: feat/smoke`; `commit_read` with `diff: true` on the new commit; `commit_compare` `base: main`, `head: feat/smoke` | The branch's commits, newest first, the new one on top; a diff as plain text; ahead by one, behind by none, one changed file. |
| 10 | `issue_open` with `labels: [bug]`, `assignees: [<you>]`; `issue_read` with `comments: true`; `issue_comment`; `issue_label`; `issue_assign` with `action: remove`; `issue_lock` then `issue_lock` `locked: false`; `issue_close` with a `comment` | Each step is visible in `issue_read`, and its comments are only what was written, not GitLab's system notes. The label `bug` is created and applied, and `not_applied` is empty. |
| 11 | `pr_open` `head: feat/smoke`, `title: Smoke`, `draft: true`; `pr_read` with `files: true` and `comments: true`; `pr_comment`; `pr_approve` | A merge request titled `Draft: Smoke`. `pr_read` shows `docs/b.md`, says it is a draft and why it cannot be merged yet. `pr_approve` is accepted or refused according to the project's approval settings; either answer is a sentence. |
| 12 | `pr_review_request` for another member, if the project has one | The reviewer is listed in `pr_read`. Skip it otherwise. |
| 13 | On `main`, `file_create` `path: .gitlab-ci.yml` with the pipeline below | A commit, which also starts a pipeline on its own. |
| 14 | `workflow_list`; `workflow_run` `ref: main`, `variables: {WORD: hi}`; `workflow_status` `ref: main`, until it is no longer `pending` or `running` | The pipeline from row 13 listed, with source `push`, and the new one with source `api`. A pipeline id. Status ends `failed`, with job `fail` and `failure_reason: script_failure`. |
| 15 | `workflow_logs` `pipeline_id: <the pipeline>` | The log of job `fail` only, ending with `this job fails on purpose` and the runner's error line, with no colour codes or `section_start` lines. `only_failed: false` adds the other job. |
| 16 | `pr_close` the draft request; `pr_open` the same branch again without `draft`; `pr_merge` `number: <the new request>`, `squash: true` | A draft cannot be merged, and GitLab has no tool here to mark it ready, so a second request is opened. It merges, with a sha when the project makes a commit for the merge; `pr_read` says merged. If the project requires a passing pipeline, the refusal says that. |
| 17 | `release_create` `tag: v0.0.1`, `target: main`; `tag_check`; `tag_get`; `tag_get_all`; `release_read`; `release_list` | The tag is made by the release. Every read finds it. |
| 18 | `release_create` `tag: v0.0.1` again | Refused, saying the release exists. |
| 19 | `release_delete` `tag: v0.0.1`; `tag_check`; `tag_delete`; `tag_check` | The release is gone and the tag is still there; then the tag is gone. |
| 20 | `release_create` `tag: v0.0.2`, `released_at` a date next year; `release_read` `tag: v0.0.2`; `release_delete` `tag: v0.0.2` | An upcoming release, found by its tag, then deleted. |
| 21 | `branch_create` `feat/x` and a file on it; `branch_merge` `base: main`, `head: feat/x`; the same merge again | A merge; the second says the base already contains the branch, and leaves no merge request behind. |
| 22 | `branch_delete` for `feat/x` and `feat/smoke`; `branch_delete` `main` | Both deleted; `main` is refused, as a protected branch. |
| 23 | `file_delete` `path: docs/a.md` | A commit; the file is gone from the tree and stays in history. |
| 24 | `repo_fork` `repo: gitlab-org/gitlab-test`; wait a moment; `repo_read` the fork | Accepted at once; readable shortly after, once its import has finished. |
| 25 | `repo_delete` with a wrong `confirm`; then with the fork's full path | The first is refused and nothing is sent; the second deletes the fork, or on gitlab.com schedules its deletion. |
| 26 | `repo_delete` `repo: <you>/scratch-delete-me`, `confirm: <you>/scratch-delete-me` | The project is gone, or scheduled to go. |

The pipeline for row 13:

```yaml
stages: [test]

say:
  stage: test
  script:
    - echo "${WORD:-hello}"

fail:
  stage: test
  script:
    - echo "this job fails on purpose" && exit 1
```

## Over HTTP

Run this once if you will use the HTTP transport. It needs only the server and a token; the walk above
then works unchanged through a client registered as in
[Set up HTTP with a token](setup-http-token.md).

| # | Do | Expect |
|---|---|---|
| H1 | `npm run start:http`, then `curl http://127.0.0.1:3002/healthz` | `{"status":"ok","server":"lxagents-gitlab","version":"…"}` with no token. |
| H2 | `curl -i -X POST http://127.0.0.1:3002/mcp` | 401 with `WWW-Authenticate: Bearer realm="mcp"`. |
| H3 | `tools/list` with `Authorization: Bearer $GITLAB_TOKEN` (the curl in the guide) | The 45 tools. |
| H4 | `tools/call` `repo_read` on a project your token can read, with that header | The project, read as the token's owner. |
| H5 | The same call with a valid but unrelated GitLab token for another account | A 404 for a private project the second account cannot see — each request acts as its own token. |
| H6 | The same call with a revoked or mistyped token | A message that GitLab rejected the token (401) and names the `Authorization: Bearer` header; the token is not in it. |
| H7 | Start the server with `MCP_AUTH_TOKEN` set and repeat H2 | A line on stderr saying it is ignored, and still a 401: the environment is never a fallback. |

## What the documentation leaves open

Each is a request shape the server sends on the strength of the documentation alone. If one behaves
differently, the tool named is the one to fix.

| Tool | What to look for |
|---|---|
| `repo_update` | `PUT /projects/:id` accepts `issues_access_level`; archiving and unarchiving answer with the project and can be repeated. |
| `repo_delete` | The status GitLab answers with (202 expected), and on gitlab.com that the project is only scheduled and a read of it then says so. |
| `repo_fork` | 201, `namespace_path` accepted in a JSON body, and `import_status` moving from `scheduled` to `finished`. |
| `repo_search` | `x-total` is sent; `order_by=similarity` works with a search. |
| `branch_create` | `branch` and `ref` are accepted as query parameters, and a tag or a sha works as `ref`; a name that exists is a 400 containing "already exists". |
| `branch_merge` | A request opened a moment ago is `checking` for a while; the merge answers 200 with `state: merged`, or 401, 405, 409 or 422 as documented; closing the request afterwards works. |
| `file_create`, `file_update`, `file_delete` | The 400 texts for an existing file, a missing file and a stale `last_commit_id` contain "already exists", "doesn't exist" and "changed since"; delete takes its parameters in a JSON body; `encoding: base64` is accepted. |
| `file_read` | A file over 10 MB is slow or rate limited. |
| `commit_list` | The answer carries `x-next-page` or a `link` header; `author` matches a name and an email; `since` and `until` take a plain date. Anonymous calls were stopped by a security check, so this is unchecked. |
| `commit_compare` | The order of `commits`, oldest or newest first; a large comparison answers `compare_timeout`. |
| `tag_check`, `tag_get` | A tag name with a slash works as `%2F` in the path; a missing tag is a 404 that does not mention the project. |
| `release_create` | A release for a missing tag creates the tag from `ref`; a second release for the tag is a 409 "Release already exists". |
| `release_read` | The permanent address for the latest release redirects with a relative `location`, followed with the token kept. |
| `issue_open`, `issue_label` | `labels` as one comma-separated string and `assignee_ids` as a list work in a JSON body; an unknown label is created; a free plan keeps the first assignee only. |
| `issue_read`, `pr_read` | `activity_filter=only_comments` drops system notes; `user_notes_count` counts people's comments only; `.../diffs` takes `per_page`; `changes_count` is a string and may be null while a request is prepared. |
| `issue_assign` | `assignee_ids: [0]` clears every assignee; `GET /users?username=` is exact and answers `[]` for an unknown name. |
| `issue_label` | `labels: ""` clears every label; `add_labels` and `remove_labels` ignore a label that is already, or not, there. |
| `issue_close`, `issue_lock`, `issue_comment` | Closing a closed issue is not an error; `discussion_locked` is a boolean once set; a locked issue refuses a non-member with a 403. |
| `pr_open` | A second request for the same source branch is a 409 saying one exists; a title starting with `Draft: ` makes `draft` true; identical branches are refused, with some wording. |
| `pr_merge` | A draft, a failing pipeline, a missing approval and a conflict each answer 405, 406 or 422, and `pr_read` says which; `squash_commit_message` is accepted; a fast-forward project leaves `merge_commit_sha` empty. |
| `pr_review_request`, `pr_approve` | `reviewer_ids` replaces the list, and a user who cannot review is dropped without an error; approving answers 201 with `approved_by` and `approvals_left`, twice is harmless, and the author is refused where the project forbids it. |
| `workflow_run` | `POST /pipeline` with `ref` and `variables: [{key, value}]` in a JSON body answers 201; the 400 text for a missing ref, for a configuration with no jobs and for variables a role may not set. |
| `workflow_status` | A private project's pipeline reads the same as a public one's, and `include_retried` is off by default. |
| `workflow_logs` | `GET /jobs/:id/trace` answers `text/plain` with a token (an anonymous call is a 401, so this has never been seen); an erased or unstarted job answers 404 or 410; `section_start` markers are removed. |
| HTTP, any tool | The token in the `Authorization` header reaches `gitlab.com` as the bearer credential of that request, and two interleaved requests with different tokens act as their own users. |
| the image | Build it, run `node src/index.js --version` in it, connect a client with `docker run --rm -i -e MCP_AUTH_TOKEN`, and check that `docker stop` returns at once. See [Docker](../environments/docker.md). |

## Reporting what differs

Open an issue on `LXAgents-MCP/gitlab` with the tool, its arguments **without the token**, and the text
of the refusal or the result. A request shape that GitLab rejects is a defect in the tool and is fixed
in a release; the behaviours above that turn out otherwise are corrected in the next version's log, not
in this one.

## Related pages

* [Connect a client](connect-a-client.md)
* [Set up HTTP with a token](setup-http-token.md)
* [Environment](../environments/env.md) — what each tool needs from the token.
* [Security model](../security/security-model.md) — why nothing has been run live.
