# Environment

**One variable, and it is a secret.**

| Variable | Default | What it does |
|---|---|---|
| `LXAGENTS_MCP_GITLAB_API_KEY` | none | The GitLab token every tool uses. Read when a tool is called, trimmed, and never taken as an argument. |

No other variable is read, there is no configuration file, and the only flags are `--help` and
`--version`. A second variable that takes a secret is a decision for the owner, not an
implementation detail: it widens what a leaked environment gives away.

## Why it is read per call

A server without a token still starts. A tool then refuses with a message that names the variable
and says where to put it, and every tool that does not need a token is unaffected. Reading at
startup would make a missing token a crash the client reports as "server failed to start", which
hides the cause.

## Where to put it

In the `env` block of this server's entry in the client's configuration, then restart the client:

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "command": "node",
      "args": ["/absolute/path/to/gitlab/src/index.js"],
      "env": { "LXAGENTS_MCP_GITLAB_API_KEY": "<your GitLab token>" }
    }
  }
}
```

Keep it out of the repository, the command line and any file that is committed. It is never logged,
never echoed in an error and never put in a result; the test suite asserts that neither a real-looking
token nor one GitLab echoes back reaches the output. `.env` and `.env.*` are gitignored.

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
| `file_read`, `directory_list` | `read_api` (`read_repository` also opens these) | Any role that can see the project |
| `file_create`, `file_update`, `file_delete` | `api` | Developer or more, and a role the branch's protection allows for a protected branch |
| `branch_merge` | `api` | Developer or more to open the request, and a role allowed to merge into the base, which is Maintainer for a protected branch |

The requirements are those GitLab documents for each endpoint. When a token is short of one, GitLab
refuses and this server repeats what it said, so the refusal is the authority if it disagrees with
this table.

## What configures the server instead

The things that are constants, in `src/config.js`, because making them settings would be a change
to a public surface: the API base (`https://gitlab.com/api/v4`), the 30 second
request timeout, the 20 MB ceiling on an answer, the 30 000 character ceiling on a result, and the
page sizes.

## Related pages

* [Security model](../security/security-model.md) — where the token can and cannot go.
* [Connect a client](../guides/connect-a-client.md)
