# Set Up the HTTP Transport with a Token

How to run this server as an HTTP service and connect a client to it with a bearer token. It serves
the same 45 tools as the stdio server; only how it is reached and how it gets the GitLab token
differ.

## When to use it

| | stdio (the default) | HTTP |
|---|---|---|
| Who starts it | The client, as a subprocess | You, once, and clients connect to it |
| Where the GitLab token goes | `MCP_AUTH_TOKEN` in the server's `env` block | `Authorization: Bearer <token>` on every request |
| Who can reach it | Only the client that spawned it | Anyone who can open its port and sends a token |
| One instance for several clients | No, one process per client | Yes, and each client sends its own token |

Use stdio when a single client runs the server on its own machine; it is simpler and needs no port.
Use HTTP when several clients should share one running server, or the server runs on another host.

**The bearer token is your GitLab token.** The server has no secret of its own and stores nothing:
it uses the token on a request for the GitLab calls of that request and forgets it. Two clients with
different tokens act as two different GitLab users, with each token's own permissions.

## 1. Create a token

On GitLab.com, create an access token with the `api` scope, or `read_api` if the agent should only read.
Prefer a project or group access token, where your plan offers them, so that the token belongs to the one
project or group the agent should touch. What each tool needs is in
[Environment](../environments/env.md#what-the-token-needs).

## 2. Start the server

From a checkout, once `npm install` has run:

```bash
npm run start:http
```

No token is given to the server, and it does not ask for one. It prints:

```text
lxagents-gitlab <version> serving over http on http://127.0.0.1:3002/mcp
a bearer token (your GitLab token) is required on every route except GET /healthz.
```

That works the same on Windows, macOS and Linux. `MCP_TRANSPORT=http npm start` does the same where
the shell accepts a variable prefix; in PowerShell set it first with `$env:MCP_TRANSPORT = "http"`.

| Variable | Default | What it does |
|---|---|---|
| `PORT` | `3002` | The port to listen on. The GitHub server defaults to `3001` and `shared-instruction` to `3000`, so all three can run together. |
| `HOST` | `127.0.0.1` | The interface to listen on. Loopback only, by default. |
| `MCP_TRANSPORT` | `stdio` | `http` (or `streamable-http`) selects this transport when you start `src/index.js`. |

```bash
PORT=8080 npm run start:http
```

**`MCP_AUTH_TOKEN` is not read over HTTP.** If it is set in the server's environment, the server says
so on stderr and ignores it; a request with no token is refused, never run with that one.

## 3. Check it with curl

```bash
curl http://127.0.0.1:3002/healthz
# {"status":"ok","server":"lxagents-gitlab","version":"<version>"}

curl -i -X POST http://127.0.0.1:3002/mcp
# HTTP/1.1 401 Unauthorized
# WWW-Authenticate: Bearer realm="mcp"

curl -X POST http://127.0.0.1:3002/mcp \
  -H "Authorization: Bearer $GITLAB_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
# {"result":{"tools":[ ...45 tools... ]},"jsonrpc":"2.0","id":1}
```

`GET /healthz` is the one route that needs no token; it says who is there and nothing else.

## 4. Register a client

Give the client the URL and the token in a header. For Claude Code:

```bash
export GITLAB_TOKEN=<your GitLab token>
claude mcp add --transport http lxagents-gitlab http://127.0.0.1:3002/mcp \
  --header "Authorization: Bearer $GITLAB_TOKEN"
```

Or commit a project `.mcp.json` that reads the token from the environment instead of holding it:

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "type": "http",
      "url": "http://127.0.0.1:3002/mcp",
      "headers": { "Authorization": "Bearer ${GITLAB_TOKEN}" }
    }
  }
}
```

The client fills in `${GITLAB_TOKEN}` from its own environment. Then **restart the client**: it loads
its server list at session start, so a server added mid-session is absent until it restarts.

**Use a variable name of your own on the client side, not `MCP_AUTH_TOKEN`.** `shared-instruction`
uses `MCP_AUTH_TOKEN` for a different secret, its own access token, and a client that expands the same
name for both entries would send one of them to the wrong server. For the same reason, set
`MCP_AUTH_TOKEN` for a stdio server only in that server's own `env` block, never exported in a shell:
a stdio process inherits its environment, and a value meant for `shared-instruction` would be sent to
GitLab as if it were a GitLab token.

## 5. Putting it on a network

The default is loopback, so only this machine can reach it. To reach it from another machine:

1. Set `HOST` to an interface that machine can reach, or to `0.0.0.0`.
2. **Put TLS in front.** The token travels in the `Authorization` header of every request, and over
   plain `http` anyone on the path can read it. This server does not terminate TLS; put a reverse proxy
   or load balancer that does (nginx, Caddy, a cloud load balancer) in front, and give clients the
   `https://` URL.
3. Treat the port as reachable by anyone: a request needs a token to do anything, and a token is only
   as private as the path it travelled.

There is no allow-list for the `Host` header and no rate limit in this server. GitLab rate limits
authenticated requests per user, and a request body is capped at 4 MB.

## 6. In a container

The image starts stdio by default. To serve HTTP from it, select the transport and bind the interface
the published port reaches:

```bash
docker run --rm -p 127.0.0.1:3002:3002 -e MCP_TRANSPORT=http -e HOST=0.0.0.0 "lxagents-gitlab:$VERSION"
```

`HOST=0.0.0.0` is for inside the container, where loopback cannot be reached through a published port;
`127.0.0.1:` in `-p` keeps it reachable from this host only. No token is passed. See
[Docker](../environments/docker.md).

## If you used the old variable

`LXAGENTS_MCP_GITLAB_API_KEY` is gone. For stdio, rename it to `MCP_AUTH_TOKEN` in the `env` block of the
server's entry. Nothing else about a stdio registration changes.

## When something is wrong

| You see | Cause | Fix |
|---|---|---|
| `401` with `WWW-Authenticate: Bearer realm="mcp"` | No `Authorization: Bearer <token>` header, another scheme, or more than one credential | Send the header with the token alone |
| `401` with `error="invalid_token"` | The credential is longer than 512 characters or has a non-ASCII character | Send the token as GitLab gave it |
| A tool says GitLab rejected the token (401) and names the `Authorization: Bearer` header | GitLab does not accept it: expired, revoked or mistyped | Create a new token |
| A tool says GitLab refused (403) | The token lacks a permission | Grant it; the message says which |
| `405` | A `GET` or `DELETE` to `/mcp` | Use `POST` |
| `406` or a client error about content types | The request lacks `Accept: application/json, text/event-stream` | Send both |
| `404` | A path other than `/mcp` | Use `/mcp` |
| `413` | A body over 4 MB | Send less |
| `could not listen on 127.0.0.1:3002` | The port is taken | Set `PORT` |
| `PORT must be a whole number` | `PORT` is not a port | Fix it |
| `MCP_AUTH_TOKEN is set ... and is ignored over HTTP` | A leftover value in the server's environment | Nothing needs doing; remove it to silence the line |

## What the server does with the token

It reads the token from the header of one request, uses it for that request's calls to
`gitlab.com`, and drops it. It is never written to the log or a result, and a message that would
contain it is scrubbed first. See [Security model](../security/security-model.md).

## Related pages

* [Environment](../environments/env.md) — the variables, and what the token needs.
* [Connect a client](connect-a-client.md) — the stdio registration.
* [Docker](../environments/docker.md)
* [Security model](../security/security-model.md)
