# Security Model

The security posture of `LXAgents-MCP/gitlab` specifically. It is scoped to this repository and
describes nothing about any other: each repository keeps its own model, and they are deliberately
not merged. In particular, the shared instruction repository holds no credential and serves public
text, so nothing it says about "nothing to protect" applies here.

**The one line worth keeping:** this server holds a GitLab key that can change and delete
projects, and the key reaches exactly one header, built in exactly one place. Over HTTP a second secret,
the access token, decides who may use the key, and it never leaves this server.

## What this project actually is, in security terms

A credentialed API client with no state. It reads no local file but its own `package.json`, runs no
command and persists nothing. Over stdio it listens on nothing; over HTTP it listens on one port, on
loopback unless told otherwise. Its input is tool arguments from a client; its
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
| `src/auth.js` `configuredApiKey`, `src/api.js` `requireToken` | Anyone who can set the process environment | Read per call; never an argument; the registry test fails any tool whose input names a credential; a caller's bearer is never used as the GitLab key |
| `src/http.js`, `src/auth.js` | The one listener, reachable by anyone who can open the port | Every route but `GET /healthz` needs `Authorization: Bearer <MCP_AUTH_TOKEN>`, compared as SHA-256 digests in constant time before the body is parsed or a path matched; HTTP will not start without a token of at least 32 characters; 4 MB body cap; binds `127.0.0.1` unless `HOST` says otherwise; a test shows the access token reaches GitLab in no part of a request |
| `src/api.js` `gitlab()` | The only place the `Authorization` header is built | One call site; the header goes to `callApi` and nowhere else |
| `src/api.js` `failure()` and `callApi` | Error text built from GitLab's answer or from the network | `redact` on every message; tests make GitLab echo the token and make the network error carry it |
| `src/server.js` `callTool` | The unexpected-exception path | `redact` on the log line and on the returned text, with the GitLab key; a test makes a handler throw the key |
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

There are two. **`GITLAB_API_KEY`** is the GitLab token, in the environment of the server process, set by
the `env` block of the client's configuration over stdio and by whoever starts the server over HTTP. It is what
every GitLab call is made with. **`MCP_AUTH_TOKEN`** is the server's access token for HTTP, in the same
environment and in each client's `headers`: it decides who may use the server and is never sent to GitLab. Neither lives anywhere
else: not in the repository, a wiki page, a test fixture, a Dockerfile `ENV` line, or a commit. The
tests use obvious dummies and no real value exists in this repository.

**The name `MCP_AUTH_TOKEN` is shared with `shared-instruction`**, which uses it for its own access token. Each
HTTP server needs its own value: one value on two servers lets whoever holds it use both. stdio never reads it,
so a value exported in a shell cannot reach GitLab.

## What to grant

Least privilege is the main control, so it is worth stating plainly. Use a project or group access
token where your plan offers them, so that it belongs to the one project or group involved, with the
lowest role that does the job and only the scope for the tools you will use; the table in
[Environment](../environments/env.md#what-the-token-needs) lists them. A token whose user cannot
delete projects cannot be talked into deleting one.

## Deployment posture

Two transports. **stdio** is the default: the client spawns the process on its own machine, and the
only caller is whoever already runs it, so there is no authentication on it and no reason for any.
**HTTP** is opt-in with `MCP_TRANSPORT=http`, and it is reachable by anyone who can open its port, so it
will not start without `MCP_AUTH_TOKEN` and every request but `GET /healthz` must carry it as
`Authorization: Bearer <token>`.

**What this costs.** Everyone who holds the access token acts as the owner of `GITLAB_API_KEY`, with all of its
permissions, `repo_delete` included, and GitLab's audit events name the key's owner, not the client. There is no
identity per caller and no way to revoke one caller without changing the token for all. That is the objection
that once kept HTTP out (`.agents/memory/decisions/stdio-only-transport.md`); the owner accepted it with the
two-token design (`.agents/memory/decisions/two-token-auth.md`). The controls are the ones in this document:
grant the key the least that works, keep the access token at 32 characters or more and private, bind
loopback, put TLS in front, run one server per person or role where identities must stay apart, and rotate by
restarting with a new token.

**What it does not do, and the operator must.** It does not terminate TLS. The access token travels in a
header, and over plain `http` anyone on the path can read it and then use the key, so the default bind is `127.0.0.1` and
anything that exposes the port must put TLS in front. It has no `Host` allow-list and no rate limit of
its own; GitLab rate limits authenticated requests per user, and a body is capped at 4 MB. Its health route says who is there and
nothing else.

A container changes where the process runs and not what it can do. The image
([Docker](../environments/docker.md)) runs as a non-root user, declares no port, and holds no secret in
any layer: the key, and over HTTP the access token, are passed at run time with `-e`, which puts them in the
container's environment, where anyone who can `docker inspect` the container sees them. The same key reaches
the same API either way.

## Verifying

| Check | How | Passing looks like |
|---|---|---|
| Neither secret appears in output | `npm test` — the api, server and http tests that make GitLab, the network and a handler echo them | all pass |
| No tool takes a credential | `npm test` — the registry test | passes |
| A path cannot leave its endpoint | `npm test` — the api tests for the four address helpers | pass |
| The token is not sent to a redirect target | `npm test` — the `callApi` redirect test | passes |
| stdout stays silent | `npm test` — the stdio tests for `--help`, `--version` and a bad flag | stdout empty |
| A request needs the access token, and the key is the server's | `npm test` — `test/http.test.js`: the 401 cases, near-miss tokens, the access token absent from every GitLab request, no upstream call without it | pass |
| HTTP will not start open | `npm test` — `test/http.test.js`: a real process with no, blank or short `MCP_AUTH_TOKEN` exits 1 | pass |
| Only one file listens | ``grep -rln -e "node:http" -e "node:net" -e "node:cluster" -e express src`` | `src/http.js` and nothing else |
| Only two runtime dependencies | `npm ls --omit=dev --depth=0` | the MCP SDK and zod |
| The image bakes in no secret | ``grep -n -e '^ENV' -e '^EXPOSE' -e '^ENTRYPOINT' Dockerfile`` | no output |

## Not a finding here

* **stdio has no authentication.** Deliberate, as above.
* **Every holder of the access token acting as the key's owner.** The design, and its cost is stated above; it is a choice, not an oversight.
* **The key sitting in the client's `env` block, or the access token in its `headers`.** That is the designed
  route; the alternative, an argument, would put it in a prompt and a transcript.
* **`GET /healthz` answering without the access token.** It is the one open route and says only that a server
  is there.
* **A registered server that reports healthy but has no tools.** The client loads connectors at
  session start, so a server added mid-session is absent until the session restarts.
* **A 404 for a project that exists.** GitLab answers 404 for a private project the token
  cannot see.

## Open

Nothing in this repository has been run with a GitLab token: the environment the work was done in
had none. Every request shape is checked against GitLab's documentation and against mocks, and a
read shape against public data on `gitlab.com` where that was possible. What a given token is actually
allowed to do is only visible in a real call, so a first smoke test against a throwaway project is the
owner's; the walk is in [Smoke test](../guides/smoke-test.md).

## Reporting something

Open an issue on `LXAgents-MCP/gitlab`. If the finding involves a token that may have leaked,
**revoke the token first**, then report.

## Summary

The whole security posture is one GitLab key handled in one place: it never leaves `src/api.js`
except as an `Authorization` header to `gitlab.com`, it is scrubbed from every message the
server can produce, no argument can carry it or redirect a call, and the one listener refuses every
request that does not bring the server's access token. What remains is authority — the key can do what it was granted, for everyone who holds the access token — which is
why the destructive tools are annotated, why `repo_delete` asks for the project's path, and why the
first advice is to grant less.
