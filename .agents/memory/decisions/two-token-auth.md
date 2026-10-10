---
name: memory-decisions-two-token-auth
description: Why this server holds two secrets, MCP_AUTH_TOKEN for the client and GITLAB_API_KEY for GitLab, and what that costs compared with passing the client's token through.
---

# Two secrets: one for the door, one for GitLab

## 2026-10-10

**Decision.** Over HTTP a client proves it may use this server with `Authorization: Bearer <MCP_AUTH_TOKEN>`,
a secret in the server's environment of at least 32 characters, compared in constant time; the server will not
start HTTP without it. Every call to GitLab, on both transports, is made with `GITLAB_API_KEY`, read per call
from the server's environment. The client's bearer is checked and dropped: it is never sent to GitLab, and a
client never supplies or sees the GitLab key. stdio has no client authentication and never reads
`MCP_AUTH_TOKEN`. The owner asked for this and chose the two names `MCP_AUTH_TOKEN` and `GITLAB_API_KEY`.

**What it replaces.** [`http-token-transport.md`](http-token-transport.md) chose to pass the client's bearer
through as the GitLab token, so that the server held no secret. That is what 0.2.0 shipped. It had one token
doing two jobs, which meant a client could not be told apart from a person holding a GitLab token, the server
had nothing of its own to protect its port, and `MCP_AUTH_TOKEN` on stdio meant something different from what it
means in `shared-instruction`.

**What it costs, which the earlier decision avoided.** Everyone who holds `MCP_AUTH_TOKEN` now acts as the
owner of `GITLAB_API_KEY`, with all of that key's permissions, `repo_delete` included, and there is no identity
per caller. This is the objection [`stdio-only-transport.md`](stdio-only-transport.md) recorded first.

| Objection | Answer now |
|---|---|
| Anyone with the server's token acts as its owner | True, and accepted by the owner. The token is at least 32 characters, compared in constant time, never logged, and the port binds `127.0.0.1` unless `HOST` says otherwise. Give `GITLAB_API_KEY` the least scope that does the job. |
| No identity per caller | True. GitLab's audit events name the owner of the key, not the client. One server per person or per role is the way to keep identities apart. |
| Cannot revoke one caller | True. Rotating `MCP_AUTH_TOKEN` and restarting cuts every caller at once. |
| TLS | Not terminated here. Anything that exposes the port must put TLS in front, because the access token is sent in a header on every request. |
| Rate limiting | None in this server. A request body is capped at 4 MB and the token is checked before the body is read. |

**What stays true.** The check comes before the body is parsed and before a path is matched, so a caller without
the token cannot make the server read 4 MB or learn which routes exist. Only `GET /healthz` is open. A wrong
token and a missing one are told apart by `error="invalid_token"` and nothing else. The token is hashed with
SHA-256 and the digests compared with `timingSafeEqual`, so neither the length nor a correct prefix shows in the
timing. `node:http` and the SDK's transport are still all that serves, and no dependency was added.

**A missing GitLab key is not fatal.** The server starts, says so on stderr over HTTP, and a tool refuses
naming `GITLAB_API_KEY`. A missing or short `MCP_AUTH_TOKEN` is fatal to HTTP, because starting without it would
be an open door.

**The name.** `MCP_AUTH_TOKEN` now means here what it means in `shared-instruction`: the secret that guards the
HTTP door. The earlier hazard, a value exported for one server and sent to GitLab by the other, is gone because
stdio does not read it. What remains is two HTTP servers given one value, which the documentation advises
against: generate one per server.

**Alternatives declined.** Keeping the passthrough as an option beside the key: two ways in, and the weaker one
would be the one that gets used. Reading the GitLab key from a request header when present: a second path by
which a caller's own key could be mixed with the server's. A `GITLAB_API_KEY` file or a secrets manager: out of
scope; the environment is what this server reads.

**If it is reopened.** Identity per caller (a map of access tokens to GitLab keys), TLS termination, a `Host`
allow-list and rate limiting are each their own task.
