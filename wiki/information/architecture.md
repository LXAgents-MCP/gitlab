# Architecture

A call goes from the client's arguments, through two checks, to one request to GitLab, and back as
text plus structured data. Everything that can go wrong on the way becomes a sentence the model can
act on.

## The tool surface is discovered, not declared

`src/tools/index.js` imports every module in `src/tools/` at startup. A module that exports `TOOL`
(its name, description, schemas and annotations) and `handle(args)` is a tool; one that exports
neither is skipped. Nothing else names a tool: `src/server.js` registers whatever was found, so the
protocol layer cannot grow a per-tool special case. A file's name is its tool's name in kebab case
— `branch-list.js` is `branch_list` — and a test holds files and tools together in both
directions. Two modules with one name stop the process at startup, not at the first call.

## A call, from argument to result

1. **The client calls a tool.** The MCP SDK checks the arguments against the tool's input schema,
   which catches a wrong type.
2. **The handler checks the values.** `parseRepo`, `refName`, `filePath` and the helpers in
   `src/args.js` refuse a bad project, a name git would reject, a `..` segment or an out-of-range
   page — before the token is read and before anything is sent.
3. **`gitlab()` builds the request.** It reads the token from the environment, builds the URL under
   the API base, and sends the token as a bearer credential with a `User-Agent`. It is the only code
   that builds a URL.
4. **`callApi` sends it.** One request, with a 30 second timeout, and at most 20 MB of the answer
   is read.
5. **The status decides.** A 2xx answer, or a status the tool listed as an answer in its own right,
   goes back to the handler. Anything else becomes a `Refused` whose message names the status and
   the next step, with the token removed.
6. **The handler returns a payload**: a one-sentence `summary` and fields that satisfy the tool's
   output schema.
7. **`toolResult` renders it.** `src/render.js` turns the payload into the text the model reads —
   scalars as bullets, a list of rows as a heading and bullets, long text as a fenced block — and
   the same fields go in `structuredContent`.

A `Refused` is an `isError` result carrying the reason and no structured content, because a client
validates structured content against the output schema and an empty object would be rejected for
the fields it lacks. Any other exception is logged to stderr and returned as an internal error, and
the server carries on. Both are scrubbed of the token.

## One server instance per connection

`McpServer` holds per-connection state, so `createServer()` builds a fresh one each time and nothing
hoists one to module scope.

## Stdio only

`src/index.js` connects a stdio transport and nothing else. There is no HTTP code and no listener,
by decision — see [Security model](../security/security-model.md). Because stdout is the protocol
channel, nothing in `src/` writes to it; diagnostics go to stderr through `src/log.js`.

The process exits by itself when its client goes away, since stdin closes. `SIGINT` and `SIGTERM`
close the server and exit cleanly; the second matters in a container, where the server is the first
process and would otherwise ignore it.

## No local path is ever read from a caller

The server reads one local file, its own `package.json`, once, to learn its version. No argument
names a local path, and no tool reads or writes the local file system or runs a command. That is
`cli`'s job.

## Related pages

* [MCP surface](../reference/mcp-surface.md) — the conventions every tool shares.
* [Environment](../environments/env.md)
