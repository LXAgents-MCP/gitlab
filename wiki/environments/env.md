# Environment

**Five variables, and two of them are secrets.** One is the GitLab key, which every tool uses; one is the
access token that guards the HTTP transport; three choose and place the HTTP transport. A client that spawns
the server over stdio sets only the key.

| Variable | Default | What it does |
|---|---|---|
| `GITLAB_API_KEY` | none | The GitLab token every tool uses, **on both transports**. Read when a tool is called, trimmed, and never taken as an argument. |
| `MCP_AUTH_TOKEN` | none, **required for HTTP** | The server's own access token: what a client must send as `Authorization: Bearer <token>` to use the HTTP transport. At least 32 characters. **HTTP will not start without it.** stdio never reads it, and it is never sent to GitLab. |
| `MCP_TRANSPORT` | `stdio` | Which transport `src/index.js` speaks: `stdio`, or `http` (`streamable-http` is accepted too). Anything else exits 2. |
| `PORT` | `3000` | The port the HTTP transport listens on, the same as `shared-instruction`. A value that is not a port exits 2. The GitHub server also defaults to `3000`: set `PORT` to run both on one machine. |
| `HOST` | `0.0.0.0` | The interface it listens on: every one, as `shared-instruction` does. `HOST=127.0.0.1` keeps it to this machine. The process holds a GitLab key, so what guards it is `MCP_AUTH_TOKEN`; see [Security model](../security/security-model.md). |

There is no configuration file, and the only flags are `--help` and `--version`. A further variable that takes
a secret is a decision for the owner, not an implementation detail: it widens what a leaked environment gives
away. `LXAGENTS_MCP_GITLAB_API_KEY` was the variable before `0.2.0` and is gone; in `0.2.0` the GitLab token was
`MCP_AUTH_TOKEN`, which now means something else (see [If you used 0.2.0](../guides/setup-http-token.md#if-you-used-020)).

## Two secrets, two jobs

| | `GITLAB_API_KEY` | `MCP_AUTH_TOKEN` |
|---|---|---|
| Direction | server → GitLab | client → server |
| Used on | stdio and HTTP | HTTP only |
| Set in | The server's environment: the `env` block of its entry in the client's configuration over stdio, the process environment over HTTP | The server's environment, and the client's `headers` for its HTTP entry |
| Who sees it | Only the server | The server and each client you give it to |
| Missing | The server starts; a tool refuses and names the variable | HTTP refuses to start |

The access token says "this client may use the server"; the key is what the server then does GitLab's work
with. A client never sees the key, and the access token is never sent to GitLab.
[Set up HTTP with two tokens](../guides/setup-http-token.md) walks through it.

**`MCP_AUTH_TOKEN` is also the name `shared-instruction` uses, for its own token.** Give each server its own
value: two servers on one value let whoever holds it use both.

## Why the key is read per call

A server without the key still starts. A tool then refuses with a message that names the variable and says
where to put it, and every tool that does not need it is unaffected. Reading at startup would make a missing
key a crash the client reports as "server failed to start", which hides the cause. The access token is the
opposite on purpose: HTTP without it would be an open door, so that is the one missing secret that stops the
server.

## Where to put them

The key, over stdio, in the `env` block of this server's entry in the client's configuration, then restart the
client:

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "command": "node",
      "args": ["/absolute/path/to/gitlab/src/index.js"],
      "env": { "GITLAB_API_KEY": "<your GitLab token>" }
    }
  }
}
```

Over HTTP the server is started by you, with both variables in its environment, and each client sends the access
token in the `headers` of its entry; see [Set up HTTP with two tokens](../guides/setup-http-token.md).

Under Docker the `env` block reaches the `docker` process and `-e GITLAB_API_KEY` passes the key into the
container; for HTTP add `-e MCP_AUTH_TOKEN`. See [Docker](docker.md).

Keep both out of the repository, the command line and any file that is committed. Neither is ever logged, echoed
in an error or put in a result; the test suite asserts that neither a real-looking key nor one GitLab echoes back
reaches the output, and that the access token never reaches GitLab. `.env` and `.env.*` are gitignored.

## What the token needs

GitLab decides what each call may do from the token and from the role of the user behind it, not
from this server, so the two together are the real limit on what an agent can change. Give the token
only what the tools you will use need.

* **The scope.** `api` allows everything the API offers the token's user, and every tool that changes
  something needs it. `read_api` allows only reads, which is enough for the read tools. Narrower
  scopes, such as `read_repository`, open only part of the API, so a tool may be refused under one.
* **The role.** What the user may do in a project or group is the role GitLab requires for the same
  action in its web interface: Developer or more to push or open a merge request, subject to any
  protected branch; Maintainer to merge into a protected branch; Owner to delete a project.
* **The kind of token.** A **project or group access token** is the least privilege GitLab offers: it
  belongs to one project or group, with its own role and scopes, where a personal access token acts
  as its owner everywhere that user can. Use one where your plan offers them.

| Tools | Scope | Role |
|---|---|---|
| `repo_create` | `api` | Allowed to create projects in the namespace: always in the token owner's own, and in a group only where the group's settings allow the user's role to. |
| `repo_read`, `repo_search` | `read_api` | Any role that can see the project; a search lists only what the token's user can see |
| `repo_update` | `api` | Maintainer; GitLab documents Owner for archiving and unarchiving |
| `repo_delete` | `api` | Owner |
| `repo_fork` | `api` | Any role that can see the project, and permission to create projects in the target namespace |
| `branch_read`, `branch_list` | `read_api` | Any role that can see the project |
| `branch_create`, `branch_delete` | `api` | Developer or more, and a role the branch's protection allows for a protected branch |
| `tag_check`, `tag_get`, `tag_get_all`, `release_read`, `release_list` | `read_api` | Any role that can see the project |
| `tag_delete`, `release_create`, `release_delete` | `api` | Developer or more; a protected tag also limits who may create it |
| `file_read`, `directory_list` | `read_api` (`read_repository` also opens these) | Any role that can see the project |
| `commit_read`, `commit_list`, `commit_compare` | `read_api` | Any role that can see the project |
| `file_create`, `file_update`, `file_delete` | `api` | Developer or more, and a role the branch's protection allows for a protected branch |
| `branch_merge` | `api` | Developer or more to open the request, and a role allowed to merge into the base, which is Maintainer for a protected branch |
| `issue_read` | `read_api` | Any role that can see the project |
| `issue_open`, `issue_comment` | `api` | Guest or more; on a locked issue only a member can comment |
| `issue_close`, `issue_assign`, `issue_label`, `issue_lock` | `api` | Planner or more; a Guest can close only an issue they wrote or are assigned to |
| `pr_read` | `read_api` | Any role that can see the project |
| `pr_comment` | `api` | Planner or more; not if merge requests are limited to project members |
| `pr_open`, `pr_close`, `pr_review_request` | `api` | Developer or more |
| `pr_approve` | `api` | Developer or more; a Reporter or Planner only where the project allows it |
| `pr_merge` | `api` | Developer or more, and a role allowed to merge into the target, which is Maintainer for a protected branch |
| `workflow_list`, `workflow_status` | `read_api` | Any role that can see the project's pipelines |
| `workflow_logs` | `read_api` | Any role that can see the project's job logs; a project may limit them to members |
| `workflow_run` | `api` | Developer or more, and a role allowed to run pipelines on a protected branch or tag; the project may limit who can set variables |

The requirements are those GitLab documents for each endpoint. When a token is short of one, GitLab
refuses and this server repeats what it said, so the refusal is the authority if it disagrees with
this table.

## What configures the server instead

The things that are constants, in `src/config/config.js`, because making them settings would be a change
to a public surface: the API base (`https://gitlab.com/api/v4`), the 30 second
request timeout, the 20 MB ceiling on an answer, the 30 000 character ceiling on a result, and the
page sizes.

## Related pages

* [Security model](../security/security-model.md) — where the token can and cannot go.
* [Connect a client](../guides/connect-a-client.md)
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
