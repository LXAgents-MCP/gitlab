# 0.3.0

**Released:** 2026-10-10

`lxagents-gitlab` now has two separate secrets. **`MCP_AUTH_TOKEN`** is the server's own access token: over
HTTP a client must send it as `Authorization: Bearer <token>`, as it does to `shared-instruction`, and HTTP
will not start without one. **`GITLAB_API_KEY`** is the GitLab token: every GitLab call is made with it, over
stdio and over HTTP, and a client never sees or supplies it. The 45 tools, their arguments and their results do
not change.

In `0.2.0` there was one secret doing both jobs: over HTTP the client's bearer token was passed through as the
GitLab token, and over stdio `MCP_AUTH_TOKEN` held the GitLab token. This release undoes that on the owner's
instruction. **`MCP_AUTH_TOKEN` therefore means something different from `0.2.0`**, and both variable names a
registration may use have changed.

**Breaking.** A stdio registration made for `0.2.0` still starts, and its first tool is refused with a message
naming `GITLAB_API_KEY`; the old value is not sent anywhere. An HTTP client made for `0.2.0` that sends a GitLab
token as its bearer is answered `401`.

**What this costs.** Everyone who holds the access token now acts as the owner of `GITLAB_API_KEY`, with all of the
key's permissions, `repo_delete` included, and GitLab's audit events name the key's owner and not the client. `0.2.0`
had one identity per caller; this has one for the whole server. Give the key the least scope that works, keep the
access token private, and run one server per person or role where identities must stay apart. See
[Security model](../../../../security/security-model.md).

**Nothing has run against live GitLab**, as in `0.1.0`, and the HTTP transport has been exercised only on
loopback with `fetch` standing in for GitLab. [Smoke test](../../../../guides/smoke-test.md) has a nine-step HTTP
walk as well as the tool walk.

**Clients must, to keep using it:**

1. **Over stdio, rename the variable.** In the `env` block of the server's entry, or in `docker run -e`, use
   **`GITLAB_API_KEY`** with the GitLab token that `0.2.0` kept in `MCP_AUTH_TOKEN`. Remove `MCP_AUTH_TOKEN` from
   that entry; stdio does not read it.
2. **To serve HTTP, give the server both variables.** Start it with `GITLAB_API_KEY` and a new `MCP_AUTH_TOKEN` of
   at least 32 characters, for example `openssl rand -hex 32`. It exits 1 without one. Give each server its own
   value: `shared-instruction` uses the same variable name for its own token.
3. **Point each HTTP client at the access token, not at a GitLab token.** Register the URL with
   `Authorization: Bearer <MCP_AUTH_TOKEN>` in `headers`, using a variable of the client's own to hold it
   (`claude mcp add --transport http lxagents-gitlab http://127.0.0.1:3002/mcp --header "Authorization: Bearer
   $LXAGENTS_GITLAB_ACCESS"`). Clients no longer hold a GitLab token.
4. **Restart the client**, so it reloads the connector.

A server without `GITLAB_API_KEY` still starts, over stdio and over HTTP; a tool then refuses with a message
naming it, and the HTTP process says so on stderr at start. A server without a usable `MCP_AUTH_TOKEN` does not
start HTTP.

## Added

- **An access token for the HTTP transport,** `MCP_AUTH_TOKEN`. Every route but `GET /healthz` needs it as
  `Authorization: Bearer <token>`, checked before the body is parsed and before a path is matched. It is hashed
  with SHA-256 and compared in constant time, so neither its length nor a correct prefix shows in the timing. A
  missing header is a `401` with `WWW-Authenticate: Bearer realm="mcp"`; a credential that does not match adds
  `error="invalid_token"`; neither answer says anything about the token.
- **A fail-closed start.** HTTP exits 1, names `MCP_AUTH_TOKEN`, shows `openssl rand -hex 32` and listens on
  nothing when the token is unset, blank or shorter than 32 characters. The floor is `shared-instruction`'s.
- **A warning at HTTP start** when `GITLAB_API_KEY` is unset, since every tool will refuse.
- **`MIN_AUTH_TOKEN_LENGTH` and `AUTH_ENV`** in `src/config.js`, and `startHttpServer` takes an `authToken` option
  that must pass the same check.
- **Tests:** 300 under `node --test`, 8 more than `0.2.0`. They cover the near-miss tokens (prefix, suffix,
  longer, wrong case, empty, the GitLab key itself), the access token absent from every request to GitLab, the
  GitLab key used whatever the client sends, no upstream call without the access token or without the key,
  neither secret in a response or the log, and the startup refusals from a real process.

## Changed

- **`GITLAB_API_KEY` is the GitLab token,** on both transports, read when a tool is called. It replaces
  `MCP_AUTH_TOKEN` in that role. The refusal for a missing key, GitLab's `401` message, the tool descriptions and
  the server's `instructions` name it.
- **`MCP_AUTH_TOKEN` is the HTTP access token.** stdio never reads it.
- **Over HTTP the client's bearer is never the GitLab token.** It is compared with `MCP_AUTH_TOKEN` and dropped.
- **The setup guide, `setup-http-token.md`, is rewritten** for two tokens, with a table for migrating from
  `0.2.0`; the environment page, the security model and the other pages that described the passthrough are
  corrected.

## Removed

- **The per-request GitLab token,** its `AsyncLocalStorage`, the process-wide "HTTP mode" guard and the 512-character
  shape check on a presented credential. They existed only because the bearer was the GitLab token.

## Security

- **The access token never reaches GitLab, the log or a result.** A test searches every request to GitLab for it.
  The GitLab key is likewise scrubbed from every message the server can produce.
- **HTTP cannot start open.** There is no setting that serves HTTP without an access token.
- **The default bind is loopback,** because the process holds a GitLab key. `HOST=0.0.0.0` is for a container
  whose published port is bound to the host's loopback.
- **There is no `Host` allow-list, rate limit or TLS** in this server. GitLab rate limits authenticated requests per user; terminate TLS in
  front, because the access token crosses the wire in a header. See
  [Security model](../../../../security/security-model.md).
- **No dependency was added.**
