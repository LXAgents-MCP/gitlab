# Local Setup

An MCP server over stdio, or over HTTP behind an access token. The only flags are `--help` and `--version`;
the transport is chosen by `MCP_TRANSPORT`.

## Requirements

Rust 1.88 or newer, with `cargo`. The server is Rust on the official MCP SDK (`rmcp`) and `axum`, built into
one binary, `target/release/lxagents-gitlab`, which is what runs and is never committed.

```bash
cargo build --release                        # target/release/lxagents-gitlab; again after every change
cargo fmt --check                            # no output
cargo clippy --all-targets -- -D warnings    # src/ and test/, no output
cargo test
```

The first build or test run in a checkout downloads and compiles every dependency, which needs the network
and takes minutes; later runs are quick.

The suite replaces the GitLab backend in each handler's context and reaches no host beyond loopback. It drives a real MCP client against the
server on an in-memory transport, spawns the real process to check what only a pipe shows (a stray
write to stdout, a process that outlives its client), and tests each tool for the exact request it
sends, the payload it returns, and the refusals it makes. It is one test binary, `test/main.rs`, whose
modules are the files under `test/`; two of them, `helpers.rs` and `surface.rs`, are support and hold no
tests.

## Run

A client normally starts the server, so "running" it means pointing the client at it — see
[Connect a client](../guides/connect-a-client.md). To run it by hand:

```bash
GITLAB_API_KEY=<token> cargo run --release
```

It then waits on stdin for a client. Press Ctrl-C to stop it, or close its stdin. Without the
variable it still starts; a tool then refuses and names the variable.

To serve HTTP on this machine only, a local install, give it the GitLab key and nothing else. It listens on
`127.0.0.1` and asks for no access token:

```bash
GITLAB_API_KEY=<token> MCP_TRANSPORT=http cargo run --release   # http://127.0.0.1:3000/mcp, this machine only
```

To serve HTTP to other machines, a server install, also give it an access token for clients (at least 32
characters; on any address but loopback it will not start without one):

```bash
GITLAB_API_KEY=<token> MCP_AUTH_TOKEN=$(openssl rand -hex 32) MCP_TRANSPORT=http cargo run --release   # port 3000, all interfaces
```

See [Set up HTTP with two tokens](../guides/setup-http-token.md). The two flags:

```bash
target/release/lxagents-gitlab --help       # usage, on stderr
target/release/lxagents-gitlab --version    # name and version, on stderr
```

## Inspect it

```bash
GITLAB_API_KEY=<token> npx @modelcontextprotocol/inspector target/release/lxagents-gitlab
```

opens the MCP Inspector against the stdio server, so you can list the tools and call one by hand.
This is the quickest way to see the surface a fresh process would build.

## stdout belongs to the protocol

On stdio, stdout is the JSON-RPC channel. One stray write corrupts the stream, and the client
reports a JSON parse error that points at the payload rather than at the logging. Nothing in `src/`
writes to stdout; diagnostics go to stderr, which is why `--help` and `--version` print there.

## Tool changes require a restart

The registry is compiled in and read once, at startup. Editing `src/controllers/` has no effect until the
binary is rebuilt: run `cargo build --release`, then restart the client or the server.

## Related pages

* [Environment](env.md) — the variables.
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
* [Docker](docker.md) — the same server in a container.
* [Architecture](../information/architecture.md)
