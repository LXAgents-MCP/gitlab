# Local Setup

An MCP server over stdio. There is no HTTP mode, and the only flags are `--help` and `--version`.

## Requirements

Node.js 20 or newer. There is no build step — the server is plain JavaScript (ESM).

```bash
npm install
npm test
```

`npm install` first, once per checkout: a fresh clone has no `node_modules`, and the suite then
fails with `ERR_MODULE_NOT_FOUND`, which reads as broken code rather than an uninstalled tree.

The suite replaces `fetch` and reaches no host beyond loopback. It drives a real MCP client against the
server on an in-memory transport, spawns the real process to check what only a pipe shows (a stray
write to stdout, a process that outlives its client), and tests each tool for the exact request it
sends, the payload it returns, and the refusals it makes. `node --test` also executes the two
support files under `test/`, which define no tests, so the runner's total includes them.

## Run

A client normally starts the server, so "running" it means pointing the client at it — see
[Connect a client](../guides/connect-a-client.md). To run it by hand:

```bash
LXAGENTS_MCP_GITLAB_API_KEY=<token> npm start
```

It then waits on stdin for a client. Press Ctrl-C to stop it, or close its stdin. Without the
variable it still starts; a tool then refuses and names the variable.

```bash
node src/index.js --help       # usage, on stderr
node src/index.js --version    # name and version, on stderr
```

## Inspect it

```bash
LXAGENTS_MCP_GITLAB_API_KEY=<token> npm run inspect
```

opens the MCP Inspector against the stdio server, so you can list the tools and call one by hand.
This is the quickest way to see the surface a fresh process would build.

## stdout belongs to the protocol

On stdio, stdout is the JSON-RPC channel. One stray write corrupts the stream, and the client
reports a JSON parse error that points at the payload rather than at the logging. Nothing in `src/`
writes to stdout; diagnostics go to stderr, which is why `--help` and `--version` print there.

## Tool changes require a restart

The registry is read once, at startup. Editing `src/tools/` has no effect on a running process:
restart the client or the server.

## Related pages

* [Environment](env.md) — the one variable.
* [Architecture](../information/architecture.md)
