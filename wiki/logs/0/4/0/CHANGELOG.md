# 0.4.0

**Released:** 2026-10-10

`lxagents-gitlab`'s HTTP transport now listens where `shared-instruction` does by default: **port 3000 on every
interface**. In `0.3.0` it listened on `127.0.0.1:3002`. The 45 tools, the two tokens and the rest of the HTTP
behaviour do not change, and stdio is untouched.

**Breaking.** A client that connected to port `3002` gets nothing from a `0.4.0` server, and a server that was
reachable only from its own machine by default is now reachable from the network by default. The second is the
one to read twice: the process holds a GitLab key. What stands in front of it is the access token, `MCP_AUTH_TOKEN`:
HTTP still refuses to start without one of at least 32 characters, and still refuses every request that does not
carry it. It does not terminate TLS, so a network you do not trust needs TLS in front.

**Nothing has run against live GitLab**, as in `0.1.0`, and the HTTP transport has been exercised only on
loopback with `fetch` standing in for GitLab.

**Clients must, to keep using it over HTTP:**

1. **Change the port in each client's URL from `3002` to `3000`,** or start the server with `PORT=3002` to keep it.
2. **If you relied on the old loopback bind, say so:** start the server with `HOST=127.0.0.1`. In a container,
   publish as `-p 127.0.0.1:3000:3000` and drop `-e HOST=0.0.0.0`, which is no longer needed.
3. **If you run this server and `lxagents-github` on one machine,** both now default to `3000`: set `PORT` on at
   least one of them.
4. **Restart the client**, so it reloads the connector.

Nothing changes over stdio.

## Changed

- **`PORT` defaults to `3000` and `HOST` to `0.0.0.0`,** the same as `shared-instruction`. `PORT` and `HOST` still
  override them; empty or blank values take the defaults.
- **The start-up line says what is bound:** `serving over http on :3000/mcp (all interfaces)` on the default, and the
  full URL when `HOST` names an interface.
- **The usage text, the header comments and the documentation no longer say loopback or `3002`.**

## Not changed, on purpose

- **A `PORT` that is set but is not a port still exits 2.** `shared-instruction` reads `Number(PORT) || 3000`, which
  quietly listens on `3000` for `PORT=abc`. Here the server refuses to start, because listening somewhere other
  than where it was asked to is worse. For every valid value the result is the same, and `PORT=0` still asks the
  system for a free port.
- **The start still fails closed** without a usable `MCP_AUTH_TOKEN`.

## Added

- **Tests:** 302 under `node --test`, 2 more than `0.3.0`. They check the defaults (unset, empty, blank), the
  overrides, `0`, five values that are not ports, and a real process on the default host and on a narrowed one.

## Security

- **The default bind is every interface.** The guard is the access token; nothing was added to it and nothing was
  weakened. Use `HOST=127.0.0.1`, or `-p 127.0.0.1:3000:3000` for the container, where only one machine needs it.
- **There is still no `Host` allow-list, rate limit or TLS** in this server. See
  [Security model](../../../../security/security-model.md).
- **No dependency was added.**
