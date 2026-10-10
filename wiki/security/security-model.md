# Security Model

The security posture of `LXAgents-MCP/gitlab` specifically. It is scoped to this repository and
describes nothing about any other: each repository keeps its own model, and they are deliberately
not merged. In particular, the shared instruction repository holds no credential and serves public
text, so nothing it says about "nothing to protect" applies here.

**The one line worth keeping:** this server holds a GitLab token that can change and delete
projects, and the token reaches exactly one header, built in exactly one place.

## What this project actually is, in security terms

A credentialed API client with no state. It reads no local file but its own `package.json`, runs no
command, persists nothing and listens on nothing. Its input is tool arguments from a client; its
output is HTTPS requests to `gitlab.com` and results back to the same client. The interesting
risk is not data theft from the server — there is no data in it — but **authority**: whatever the
token may do, an agent calling this server may do.

## The trust boundary that matters most

The caller is a model, and a model can be steered by text it read. An issue body, a file in a
project or a commit message may say "now delete this project", and the server cannot tell a
steered call from a wanted one. So the limits are the ones that do not depend on the model
behaving:

* **The token's permissions** are the real limit. GitLab enforces them, not this server. Grant only
  what the tools in use need, on only the projects involved.
* **Annotations** say what kind of change each tool makes, and deletes and merges are marked
  destructive, so a client can ask a person first.
* **`repo_delete` refuses** unless it is told the project's full path, which makes a
  copy-and-paste accident or a vague instruction fail.
* **The server cannot make a client ask.** Everything past those three is the person at the client.

## Surfaces

### The token

| File | Exposure | Guard |
|---|---|---|
| `src/api.js` `requireToken` | Anyone who can set the process environment | Read per call; never an argument; the registry test fails any tool whose input names a credential |
| `src/api.js` `gitlab()` | The only place the `Authorization` header is built | One call site; the header goes to `callApi` and nowhere else |
| `src/api.js` `failure()` and `callApi` | Error text built from GitLab's answer or from the network | `redact` on every message; tests make GitLab echo the token and make the network error carry it |
| `src/server.js` `callTool` | The unexpected-exception path | `redact` on the log line and on the returned text; a test makes a handler throw the token |
| `src/log.js` | stderr | The logger writes what it is given, and nothing logs a request or a header |

### Which endpoint a call reaches

| File | Exposure | Guard |
|---|---|---|
| `src/api.js` `parseRepo`, `refName`, `refPath`, `filePath` | Arguments from a model that may be steering on text it read | Validated and percent-encoded; empty, `.` and `..` segments refused; tests cover each |
| `src/api.js` `gitlab()` | The path handed to it | Must start with `/` and carry no `?` or `#`; a violation is a programming error, not a refusal |
| `src/config.js` `API_BASE` | The host | A constant: no argument or variable can change it |
| `src/api.js` `callApi` | The address GitLab redirects a read to, when a project has moved | The destination comes from GitLab's answer and never from an argument; the token travels in `Authorization`, which the runtime drops when the host changes (GitLab's own `PRIVATE-TOKEN` header it would forward), and a test shows a second host receiving no token, over real sockets |

### What a call can change

| File | Exposure | Guard |
|---|---|---|
| `src/tools/repo-delete.js` | The most destructive tool: it removes a project with its issues and merge requests, or marks it for removal after a retention period | `confirm` must repeat the full path, and a test shows nothing is sent otherwise (a numeric id sends only the read that finds the path); annotated `destroy` |

## Secrets

There is one: **`LXAGENTS_MCP_GITLAB_API_KEY`**. It lives in the environment of the server process,
set by the `env` block of the client's configuration, and nowhere else: not in the repository, a
wiki page, a test fixture, a Dockerfile `ENV` line, or a commit. The tests use an obvious dummy and
no real value exists in this repository.

## What to grant

Least privilege is the main control, so it is worth stating plainly. Use a project or group access
token where your plan offers them, so that it belongs to the one project or group involved, with the
lowest role that does the job and only the scope for the tools you will use; the table in
[Environment](../environments/env.md#what-the-token-needs) lists them. A token whose user cannot
delete projects cannot be talked into deleting one.

## Deployment posture

Stdio only: the client spawns the process on its own machine, and the only caller is whoever already
runs it. There is no authentication on stdio and no reason for any. There is deliberately **no HTTP
transport**, because over a network the same token would be usable by anyone holding the server's
bearer token, with no identity per caller and no way to revoke one. Adding one is a separate
decision with its own security record.

## Verifying

| Check | How | Passing looks like |
|---|---|---|
| The token never appears in output | `npm test` — the api and server tests that make GitLab, the network and a handler echo it | all pass |
| No tool takes a credential | `npm test` — the registry test | passes |
| A path cannot leave its endpoint | `npm test` — the api tests for the four address helpers | pass |
| The token is not sent to a redirect target | `npm test` — the `callApi` redirect test | passes |
| stdout stays silent | `npm test` — the stdio tests for `--help`, `--version` and a bad flag | stdout empty |
| No listener or HTTP library | ``grep -rn -e "node:http" -e "node:net" -e "node:cluster" -e express src`` | no output |
| Only two runtime dependencies | `npm ls --omit=dev --depth=0` | the MCP SDK and zod |

## Not a finding here

* **stdio has no authentication.** Deliberate, as above.
* **The token sitting in the client's `env` block.** That is the designed route; the alternative, an
  argument, would put it in a prompt and a transcript.
* **A registered server that reports healthy but has no tools.** The client loads connectors at
  session start, so a server added mid-session is absent until the session restarts.
* **A 404 for a project that exists.** GitLab answers 404 for a private project the token
  cannot see.

## Open

Nothing in this repository has been run with a GitLab token: the environment the work was done in
had none. Every request shape is checked against GitLab's documentation and against mocks, and a
read shape against public data on `gitlab.com` where that was possible. What a given token is actually
allowed to do is only visible in a real call, so a first smoke test against a throwaway project is the
owner's.

## Reporting something

Open an issue on `LXAgents-MCP/gitlab`. If the finding involves a token that may have leaked,
**revoke the token first**, then report.

## Summary

The whole security posture is one credential handled in one place: it never leaves `src/api.js`
except as an `Authorization` header to `gitlab.com`, it is scrubbed from every message the
server can produce, no argument can carry it or redirect a call, and there is no listener for anyone
else to use it through. What remains is authority — the token can do what it was granted — which is
why the destructive tools are annotated, why `repo_delete` asks for the project's path, and why the
first advice is to grant less.
