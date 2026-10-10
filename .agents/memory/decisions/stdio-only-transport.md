---
name: memory-decisions-stdio-only-transport
description: Superseded by http-token-transport.md. Why this server was stdio only — the token it holds can delete projects.
---

# This server is stdio only

> **Superseded on 2026-10-10** by [`http-token-transport.md`](http-token-transport.md): the owner reopened
> this, and the bearer token passed through per request answers the objections below. Kept as history.

## 2026-10-10

**Decision.** `lxagents-gitlab` speaks MCP over stdio and nothing else. It has no HTTP transport,
no listener, no express and no authentication scheme of its own. The client starts the process and
talks to it through a pipe.

**Why.** The server holds a GitLab token, and the token it is given can change and delete
projects. Over stdio the only caller is the client that spawned the process, which already runs
as the person who owns the token. An HTTP listener would let anyone holding its bearer token act as
that person, from a machine they may never touch, with no identity per caller and no way to revoke
one caller without revoking all. `shared-instruction` serves HTTP safely because its content is
public and its token only gates who may read it; the same design here would gate the ability to
delete projects, which is a different question.

**Alternatives declined.** Stdio plus HTTP, as `shared-instruction` has: declined for the reason
above. HTTP bound to loopback only: declined, because a reverse proxy on the same host makes
loopback reachable from outside, the same argument `shared-instruction` records for its own token.
The owner chose the stdio-only default with the plan.

**If it is reopened.** It is its own task with its own security record, not an edit. The bearer
design, identity per caller, TLS and rate limiting all need an answer first, and the owner decides.
Nothing in this repository should grow a listener on the way to something else.
