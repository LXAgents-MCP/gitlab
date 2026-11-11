---
name: memory-decisions-local-install-without-token
description: Why a local install of the HTTP transport (loopback only) may run without MCP_AUTH_TOKEN, how local is told from server, and what stands in for the token on loopback.
---

# A local install needs no access token

## 2026-11-11

**Decision.** The HTTP transport needs `MCP_AUTH_TOKEN` only when other machines can reach it. Listening on a
loopback address (`127.0.0.0/8`, `::1`, `localhost`) with no token set is a local install, and it starts and serves
without one. Any other address, `0.0.0.0` included, still refuses to start without a token of 32 characters or more.
With `HOST` unset, the bind follows the token: `0.0.0.0` when it is set, as before, and `127.0.0.1` when it is not.
A token that is set is always checked and always required, on loopback too. The owner asked for this. It narrows
the fail-closed start of `two-token-auth.md` and `http-bind-defaults.md` to server installs; neither record is
otherwise changed.

**Why.** stdio never needed the token, but running HTTP on one's own machine did, for no one: a secret nobody else
could present to a port nobody else could reach. Reading "local" from the bind address needs no new variable, and
it cannot be wrong in the dangerous direction: a server that says it is local while listening everywhere is not
possible, because the address is what the check reads.

**What stands in for the token on loopback.** Any web page open on this machine can send requests to a loopback
port, and DNS rebinding lets one read the answers. Without a token, a request must name loopback in `Host` (as
`localhost`, `127.x.x.x` or `[::1]`, with or without a port), and in `Origin` if it sends one, or it is a `403`
before its body is read. A browser always sends the name the page used in `Host`, so a rebound name is refused, and
a page from another site is refused by its `Origin`. A client that is not a browser sends a loopback `Host` and no
`Origin`, and passes. `GET /healthz` stays open, as it is with a token.

**What it costs.**

| Cost | Answer |
|---|---|
| Any program running on this machine, run by any user, can use the GitLab key through a local install | On a machine only its owner uses, those programs already run as the owner. A shared machine should set `MCP_AUTH_TOKEN`, which turns the check back on. |
| A container run without a token listens on its own loopback, which `-p` cannot reach | It says so at start, and a container is a server install: the documentation says to set the token. It fails closed, never open. |
| Someone who moved a token-less server onto loopback gains an unauthenticated port | Only by setting `HOST` to loopback and removing the token, both deliberate. |

**Alternatives declined.** A new variable such as `MCP_INSTALL=local` (a new public variable, and one that could
claim local while listening everywhere). Loopback by default for everyone (moves servers that have a token and rely
on `0.0.0.0`). Keeping `0.0.0.0` for everyone (a local install would need `HOST=127.0.0.1` to skip the token, which
is still a variable to set). No `Host` check (leaves the port open to any web page through DNS rebinding).
