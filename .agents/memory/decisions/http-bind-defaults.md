---
name: memory-decisions-http-bind-defaults
description: Why the HTTP transport defaults to port 3000 on all interfaces, as shared-instruction does, instead of loopback on 3002, and what guards a server that listens everywhere by default.
---

# HTTP binds every interface on port 3000 by default

## 2026-10-10

**Decision.** With neither `PORT` nor `HOST` set, the HTTP transport listens on port 3000 on `0.0.0.0`, the same
defaults as `shared-instruction`. `PORT` and `HOST` still override, and `HOST=127.0.0.1` narrows it to this
machine. The owner asked for this. It replaces D5 and D6 of `http-token-auth.md` (loopback by default; port
3002 so three servers can share a machine).

**Why.** One way to reach all three servers: the same port, the same bind, and a container whose published port
works without `-e HOST=0.0.0.0`. Loopback by default made the container case and the remote case each need a
setting to be undone.

**What guards it.** Binding every interface by default puts the port on the network of whatever runs it, and the
process holds a GitLab key. The guard is the one already in place: HTTP refuses to start without `MCP_AUTH_TOKEN` of
at least 32 characters, and every route but `GET /healthz` needs it, compared in constant time before anything
is read. Nothing was added, because a second guard would be a second thing to forget. What the default does not
do is terminate TLS: the access token crosses the wire in a header, so anything that faces a network the operator
does not trust must be behind TLS. The documentation says to publish the container as
`-p 127.0.0.1:3000:3000` when only this host should reach it.

**What it costs.**

| Cost | Answer |
|---|---|
| A server started without thinking about it is reachable from the network | It is also refused every request without the access token and will not start without one. The first line it logs says `(all interfaces)`. |
| The GitHub and GitLab servers now share port 3000 | Set `PORT` on at least one to run both on a machine. The documentation says so. |
| Anyone who reads the access token off an unencrypted path can use the key | TLS in front; unchanged from the two-token decision. |

**Not copied from `shared-instruction`: its `PORT` parsing.** It reads `Number(process.env.PORT) || 3000`, so
`PORT=abc` and `PORT=0` both silently become 3000. Here a `PORT` that is set but is not a whole number from 0 to
65535 still exits 2, because listening somewhere other than where the operator asked is worse than refusing to
start, and `PORT=0` still asks the system for a free port, which the tests use. For every valid value and for
unset or blank, the result is the same as `shared-instruction`'s.

**Alternatives declined.** Copying the `PORT` expression exactly: silent fallbacks, and no way to ask for a free
port. Keeping loopback and only changing the port: not what was asked, and it leaves the container needing a
setting. Adding a `Host` allow-list or a rate limit at the same time: each is its own task.
