---
name: memory-decisions-http-token-transport
description: Partly superseded by two-token-auth.md. Why this server also serves HTTP, and the passthrough design of 0.2.0 that answered the objections that kept it stdio only.
---

# This server serves HTTP, and the bearer token is the GitLab token

> **Partly superseded on 2026-10-10** by [`two-token-auth.md`](two-token-auth.md): the HTTP transport stays, but
> the bearer token is now the server's own `MCP_AUTH_TOKEN`, and GitLab is called with `GITLAB_API_KEY`. The
> passthrough described below is what 0.2.0 shipped. Kept as history.

## 2026-10-10

**Decision.** `lxagents-gitlab` speaks stdio by default and HTTP when `MCP_TRANSPORT=http`. Over HTTP
every route but `GET /healthz` needs `Authorization: Bearer <GitLab token>`, and that token is used for
the GitLab calls of that one request and kept nowhere. The one variable is `MCP_AUTH_TOKEN`: over stdio
it holds the GitLab token, and over HTTP it is never read. This replaces
[`stdio-only-transport.md`](stdio-only-transport.md); the owner reopened that decision.

**Why this answers the objections recorded there.** That record kept HTTP out because a listener would
let anyone holding the server's bearer token act as the person who owns the GitLab token, with no
identity per caller and no way to revoke one caller alone. Passing the GitLab token through removes the
server-held secret those objections were about:

| Objection | Answer |
|---|---|
| Anyone with the server's token acts as its owner | The server has no GitLab token of its own. A caller acts as whoever's token it sends, with that token's permissions, and GitLab's own audit events name it. |
| No identity per caller | The identity is the GitLab token, so there is one per caller. |
| Cannot revoke one caller | Revoking a GitLab token ends that caller and no other. |
| TLS | Not terminated here. The default bind is `127.0.0.1`; anything that exposes the port must put TLS in front, because a GitLab token sent in clear text can be read on the path. |
| Rate limiting | GitLab rate limits authenticated requests per user. The server adds a 4 MB body cap and checks the token's shape before it reads a body, which is all it can honestly add. |
| The listener is reachable without a token | Only `GET /healthz`, which answers who is there and nothing else. |

**What stays true.** An environment token is never an HTTP fallback, or a caller with no token would act
with the owner's. A process that serves HTTP says so to the code that fetches the token
(`enterHttpMode` in `src/auth.js`), so this does not depend on a request context surviving. The token is
checked for shape only, with no length floor: `shared-instruction` refuses under 32 characters, which
would refuse a valid GitLab token (26 characters), and GitLab is the one that knows whether a token is valid.

**The name.** `MCP_AUTH_TOKEN` is the name `shared-instruction` uses for a different secret, by the
owner's instruction. A value exported in a shell for one would be inherited by a stdio process of the
other and sent to GitLab, so the documentation says to set it only in a server's own `env` block.

**Alternatives declined.** A separate server access token beside a GitLab token in the environment: two
secrets, and still an environment. Removing stdio: it breaks every registration and `docker run -i`.
Express: the SDK's transport runs on `node:http`, so no dependency is added.

**If it is reopened.** Per-IP rate limiting, a `Host` allow-list and TLS termination are the things not
done here, and each is its own task.
