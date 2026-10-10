# Set Up the HTTP Transport with Two Tokens

How to run this server as an HTTP service and connect a client to it. It serves the same 45 tools as the
stdio server. Two secrets are involved, and they do different jobs:

| Secret | Variable | Who holds it | What it does |
|---|---|---|---|
| **Access token** | `MCP_AUTH_TOKEN` | The server, and each client you give it to | Lets a client use this server. Sent as `Authorization: Bearer <token>`. Never goes to GitLab. |
| **GitLab key** | `GITLAB_API_KEY` | The server only | The GitLab token every tool call is made with. A client never sees it. |

A client proves it may use the server with the access token; the server then acts on GitLab with its own key.
Every client that holds the access token acts as the owner of the key, with all of the key's permissions —
see [What this means for who can do what](#what-this-means-for-who-can-do-what).

## When to use it

| | stdio (the default) | HTTP |
|---|---|---|
| Who starts it | The client, as a subprocess | You, once, and clients connect to it |
| GitLab key | `GITLAB_API_KEY` in the server's `env` block | `GITLAB_API_KEY` in the server's environment |
| Access token | Not used | `MCP_AUTH_TOKEN`, required |
| Who can reach it | Only the client that spawned it | Anyone who can open its port and holds the access token |
| One instance for several clients | No, one process per client | Yes, and they share the one key |

Use stdio when a single client runs the server on its own machine; it is simpler and needs no port or access
token. Use HTTP when several clients should share one running server, or the server runs on another host.

## 1. Create the GitLab key

On GitLab.com, create an access token with the `api` scope, or `read_api` if the agent should only read. Prefer a
project or group access token, where your plan offers them, with the lowest role that does the job, over a personal
access token, which acts as its owner everywhere. What each tool needs is in
[Environment](../environments/env.md#what-the-token-needs). Give it the least that does the job: every client of
this server gets all of it.

## 2. Create the access token

Make a secret of at least 32 characters. One way, which gives 64:

```bash
openssl rand -hex 32
```

Keep it somewhere safe: the server needs it, and so does every client. **Make a separate one for each server.**
`shared-instruction` uses the same variable name, `MCP_AUTH_TOKEN`, for its own token; do not give the two
servers the same value, or whoever holds one can use the other.

## 3. Start the server

Set both variables in the server's environment, then start it. From a checkout, once `npm install` has run:

```bash
export GITLAB_API_KEY=<your GitLab token>
export MCP_AUTH_TOKEN=<the access token from step 2>
npm run start:http
```

It prints:

```text
lxagents-gitlab <version> serving over http on http://127.0.0.1:3002/mcp
the access token in MCP_AUTH_TOKEN is required on every route except GET /healthz.
```

**It will not start without a usable access token.** With `MCP_AUTH_TOKEN` unset, empty or shorter than 32
characters it exits 1, says which, and listens on nothing. A missing `GITLAB_API_KEY` is not fatal: the server
starts, says so on stderr, and every tool refuses naming the variable until it is set.

`MCP_TRANSPORT=http npm start` does the same where the shell accepts a variable prefix; in PowerShell set
`$env:MCP_TRANSPORT = "http"` first.

| Variable | Default | What it does |
|---|---|---|
| `GITLAB_API_KEY` | none | The GitLab token. Required for any tool to work. |
| `MCP_AUTH_TOKEN` | none, **required** | The access token. At least 32 characters. HTTP will not start without it. |
| `PORT` | `3002` | The port to listen on. The GitHub server defaults to `3001` and `shared-instruction` to `3000`, so all three can run together. |
| `HOST` | `127.0.0.1` | The interface to listen on. Loopback only, by default. |
| `MCP_TRANSPORT` | `stdio` | `http` (or `streamable-http`) selects this transport when you start `src/index.js`. |

```bash
PORT=8080 npm run start:http
```

## 4. Check it with curl

```bash
curl http://127.0.0.1:3002/healthz
# {"status":"ok","server":"lxagents-gitlab","version":"<version>"}

curl -i -X POST http://127.0.0.1:3002/mcp
# HTTP/1.1 401 Unauthorized
# WWW-Authenticate: Bearer realm="mcp"

curl -X POST http://127.0.0.1:3002/mcp \
  -H "Authorization: Bearer $MCP_AUTH_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
# {"result":{"tools":[ ...45 tools... ]},"jsonrpc":"2.0","id":1}
```

`GET /healthz` is the one route that needs no token; it says who is there and nothing else. A wrong token is
also a 401, with `error="invalid_token"` added to the challenge.

## 5. Register a client

Give the client the URL and the **access token** in a header. The client never needs the GitLab key. For
Claude Code:

```bash
export LXAGENTS_GITLAB_ACCESS=<the access token>
claude mcp add --transport http lxagents-gitlab http://127.0.0.1:3002/mcp \
  --header "Authorization: Bearer $LXAGENTS_GITLAB_ACCESS"
```

Or commit a project `.mcp.json` that reads the token from the environment instead of holding it:

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "type": "http",
      "url": "http://127.0.0.1:3002/mcp",
      "headers": { "Authorization": "Bearer ${LXAGENTS_GITLAB_ACCESS}" }
    }
  }
}
```

The client fills in `${LXAGENTS_GITLAB_ACCESS}` from its own environment. Then **restart the client**: it loads
its server list at session start, so a server added mid-session is absent until it restarts.

## 6. Putting it on a network

The default is loopback, so only this machine can reach it. To reach it from another machine:

1. Set `HOST` to an interface that machine can reach, or to `0.0.0.0`.
2. **Put TLS in front.** The access token travels in the `Authorization` header of every request, and over
   plain `http` anyone on the path can read it and then act as the owner of the GitLab key. This server does
   not terminate TLS; put a reverse proxy or load balancer that does (nginx, Caddy, a cloud load balancer) in
   front, and give clients the `https://` URL.
3. Treat the port as reachable by anyone: a request needs the access token to do anything, and the token is only
   as private as the path it travelled.

There is no allow-list for the `Host` header and no rate limit in this server. GitLab limits the key, and a
request body is capped at 4 MB.

## 7. In a container

The image starts stdio by default. To serve HTTP from it, select the transport, bind the interface the
published port reaches, and pass both variables by name so the values never sit in the command:

```bash
docker run --rm -p 127.0.0.1:3002:3002 -e MCP_TRANSPORT=http -e HOST=0.0.0.0 \
  -e GITLAB_API_KEY -e MCP_AUTH_TOKEN "lxagents-gitlab:$VERSION"
```

`HOST=0.0.0.0` is for inside the container, where loopback cannot be reached through a published port;
`127.0.0.1:` in `-p` keeps it reachable from this host only. See [Docker](../environments/docker.md).

## What this means for who can do what

The access token says "this client may use the server"; it does not say who the client is. Every client that
holds it acts as the owner of `GITLAB_API_KEY`: the same permissions, the same identity in GitLab's audit log,
`repo_delete` included. To keep identities or permissions apart, run one server per person or per role, each
with its own key and its own access token. To cut every client off at once, change `MCP_AUTH_TOKEN` and
restart the server.

## If you used 0.2.0

Both variables changed again.

| In 0.2.0 | Now |
|---|---|
| stdio: `MCP_AUTH_TOKEN` held the GitLab token | stdio: put the GitLab token in **`GITLAB_API_KEY`**. `MCP_AUTH_TOKEN` is not read over stdio. |
| HTTP: the client's bearer token was the GitLab token | HTTP: the bearer is the server's **`MCP_AUTH_TOKEN`**; the GitLab token goes in the server's `GITLAB_API_KEY`. Clients no longer hold a GitLab token. |
| `LXAGENTS_MCP_GITLAB_API_KEY` | Gone since 0.2.0. Use `GITLAB_API_KEY`. |

A stdio registration still carrying `MCP_AUTH_TOKEN` starts, and its first tool is refused with a message naming
`GITLAB_API_KEY`; the old value is not sent anywhere.

## When something is wrong

| You see | Cause | Fix |
|---|---|---|
| The server exits 1 saying `will not start the HTTP transport` | `MCP_AUTH_TOKEN` is unset, empty or under 32 characters | Set it to a secret of at least 32 characters |
| `401` with `WWW-Authenticate: Bearer realm="mcp"` | No `Authorization: Bearer <token>` header, another scheme, or more than one credential | Send the header with the access token alone |
| `401` with `error="invalid_token"` | The credential is not the server's `MCP_AUTH_TOKEN` | Send the value the server was started with; check for a trailing newline or a stale copy |
| A tool says `GITLAB_API_KEY is not set` | The server has no GitLab key | Set `GITLAB_API_KEY` in the server's environment and restart it |
| A tool says GitLab rejected the token (401) and names `GITLAB_API_KEY` | GitLab does not accept the key: expired, revoked or mistyped | Create a new key |
| A tool says GitLab refused (403) | The key lacks a permission | Grant it; the message says which |
| `405` | A `GET` or `DELETE` to `/mcp` | Use `POST` |
| `406` or a client error about content types | The request lacks `Accept: application/json, text/event-stream` | Send both |
| `404` | A path other than `/mcp` | Use `/mcp` |
| `413` | A body over 4 MB | Send less |
| `could not listen on 127.0.0.1:3002` | The port is taken | Set `PORT` |
| `PORT must be a whole number` | `PORT` is not a port | Fix it |
| The log says `GITLAB_API_KEY is not set in this process's environment` | The server was started without a key | Set it and restart; tools refuse until then |

## What the server does with the two tokens

It compares the access token of each request with `MCP_AUTH_TOKEN` in constant time and drops it; the token is
never sent to GitLab, never written to the log or a result. It reads `GITLAB_API_KEY` when a tool is called and
uses it for the GitLab calls; a message that would contain either secret is scrubbed first. See
[Security model](../security/security-model.md).

## Related pages

* [Environment](../environments/env.md) — the variables, and what the key needs.
* [Connect a client](connect-a-client.md) — the stdio registration.
* [Docker](../environments/docker.md)
* [Security model](../security/security-model.md)
