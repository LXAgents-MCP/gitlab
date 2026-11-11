# 0.6.0

**Released:** 2026-11-11

**A local install of `lxagents-gitlab`'s HTTP transport no longer needs `MCP_AUTH_TOKEN`.** stdio never read it, but
HTTP refused to start without one even when nothing beyond this machine could reach it. Now the server tells a
**local install** from a **server install** by where it listens:

| | Local install | Server install |
|---|---|---|
| Where it listens | Loopback: `127.0.0.0/8`, `::1` or `localhost` | Anywhere else, `0.0.0.0` included |
| `MCP_AUTH_TOKEN` | Optional. If set, every request must carry it | **Required**, at least 32 characters, as before |
| With `HOST` unset | `127.0.0.1` when no token is set | `0.0.0.0` when a token is set, as before |

So `GITLAB_API_KEY=<token> npm run start:http` with no other variable now serves `http://127.0.0.1:3000/mcp` to this
machine and says `local install` at start. Without a token, a request must name loopback in `Host`, and in `Origin`
when it sends one, or it is a `403` before its body is read; that keeps a web page open on this machine from reaching
the port through DNS rebinding. With `HOST` set to an address other machines can reach and no token, the server
still exits 1, and now says that a local install can leave `HOST` unset. **Every configuration that worked in
`0.5.0` works the same:** a token that is set still binds `0.0.0.0` by default and is still required on every
request.

**Clients must:**

1. **Nothing**, if you already run HTTP with `MCP_AUTH_TOKEN`, or run stdio. Restart the client as usual.
2. **To run HTTP on your own machine without a token,** start it with neither `MCP_AUTH_TOKEN` nor `HOST` set,
   register `http://127.0.0.1:3000/mcp` (or `http://localhost:3000/mcp`) with no `Authorization` header, and
   restart the client. On a machine other people use, keep the token.
3. **In a container, keep passing `-e MCP_AUTH_TOKEN`:** a container is a server install. Without the token the
   server inside listens on the container's own loopback, which a published port cannot reach.

## Added

- **Local installs.** `src/routes/http.js` starts without an access token when it listens on loopback, and
  `localRequestProblem` refuses with `403` a request whose `Host` or `Origin` is not loopback. `isLoopbackHost` in
  `src/services/auth.js` matches the literal names only, never what a name resolves to.
- **`LOCAL_HOST`** (`127.0.0.1`) in `src/config/config.js`: the bind when neither `HOST` nor `MCP_AUTH_TOKEN` is set.
- **Tests:** 310 pass under `node --test`, 302 in `0.5.0`: the two `HOST` defaults, the start refused off
  loopback without a token, a local install serving with no token, the `Host` and `Origin` refusals, and a real
  process with neither variable set.
- **Documentation:** a section "Local install or server install" in the HTTP guide, a local quick start in the
  README and the setup page, smoke-test row H1b, and the decision
  `.agents/memory/decisions/local-install-without-token.md`.

## Changed

- **`HOST` defaults to `127.0.0.1` when `MCP_AUTH_TOKEN` is unset**, and to `0.0.0.0` when it is set, as before.
- **The refusal to start** names the address and says how to make a local install instead.
- **The start-up log** says `local install` and what it means when there is no token.
- **The repository rules** forbid HTTP starting without a token on an address that is not loopback, where they
  forbade it everywhere, on the owner's instruction.

## Not changed, on purpose

- **No new variable.** Local or server is read from where the server listens, so a server cannot claim to be local
  while listening everywhere.
- **A token that is set is always enforced,** on loopback too, and one under 32 characters still exits 1.
- **stdio, the 45 tools, `GITLAB_API_KEY`, the routes and the image** are as they were.

## Security

- **A server install is exactly as guarded as in `0.5.0`.** HTTP on an address other than loopback still will not
  start without a token of at least 32 characters, and the comparison is unchanged.
- **A local install has no access token.** Any program on this machine can use the GitLab key through it; on a
  machine other people use, set the token. A web page cannot, because of the `Host` and `Origin` checks.
- **No dependency was added,** and `src/routes/http.js` is still the only module that loads a network module.
