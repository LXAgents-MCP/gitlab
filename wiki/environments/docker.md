# Docker

The server runs in a container over **stdio** by default, and over HTTP when told to. The image is for a
client that can spawn `docker run -i`, for one that connects to a published port, and for a host that
has no Rust toolchain.

> **This image has not been built through.** For 0.8.0 the build stage was run as written, in a
> `rust:1.97-alpine` container: `apk add` of its tools, then `cargo build --release --locked`, which produced a
> static musl binary that ran and printed its name and version. The runtime stage
> was not: the environment could not reach Alpine's package mirror from inside a container, so
> `apk add ca-certificates`, `COPY --from` and `USER nobody` in a real image are unchecked. `SIGTERM` is tested
> on the binary. Build it once and run the check at the end of this page before relying on it.

## What a container is for here

* **A pinned toolchain.** `rust:1.97-alpine` to build and `alpine:3.22` to run, instead of whatever is on the
  host. For exact reproducibility, pin both base images by digest.
* **Two stages.** The first compiles the binary; the second holds only that binary and the CA certificates
  it needs to reach GitLab over TLS, so neither the sources nor the toolchain are in the image.
* **A clean dependency tree.** `cargo build --release --locked` from `Cargo.lock`, which fails rather than
  resolve something the lockfile does not contain.
* **A non-root process.** The image ends as `USER nobody`.
* **Isolation from the host.** The server reads no host file and runs no command either way; the
  container also keeps it off the host's file system altogether.

What it does not change: the token still reaches GitLab with whatever permissions it was granted, and
it is in the container's environment, where anyone who can `docker inspect` the container sees it.

## Build

```bash
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
docker build -t "lxagents-gitlab:$VERSION" .
```

Tag it with the version in `Cargo.toml` rather than `latest`. The image's job is to be reproducible,
and `latest` is the one tag that cannot be.

## Run

```bash
docker run --rm -i -e GITLAB_API_KEY "lxagents-gitlab:$VERSION"
```

**`-i` is not optional.** Without it Docker attaches no stdin, the server sees it closed and exits
at once, which reads as a broken image rather than a missing flag. **Do not add `-t`:** a terminal
rewrites line endings on the protocol channel.

`-e GITLAB_API_KEY` with no value passes the variable through from the environment of the
`docker` process, so the key is not on the command line, in shell history or in the image. Never put
it in an `ENV` line: a value baked into the image ships to everyone who pulls it.

Over stdio there is nothing to publish. To serve HTTP from the same image, see [Over HTTP](#over-http) below.

## Register it with a client

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "-e", "GITLAB_API_KEY", "lxagents-gitlab:<version>"],
      "env": { "GITLAB_API_KEY": "<your GitLab token>" }
    }
  }
}
```

Both are needed: the `env` block gives the variable to the `docker` process, and `-e` passes it on
into the container. Replace `<version>` with the tag you built. The container needs outbound HTTPS to
GitLab, which Docker's default network gives it.

When the client goes away it closes the pipe and the container exits by itself. `docker stop` ends it
at once: the server handles `SIGTERM`, which it has to, because as the container's first process it
would otherwise ignore it and be killed after the grace period.

## Over HTTP

The image's default is stdio, so serving HTTP is a matter of selecting the transport, publishing the port, and
passing both secrets by name, so the values are not on the command line:

```bash
docker run --rm -p 127.0.0.1:3000:3000 -e MCP_TRANSPORT=http \
  -e GITLAB_API_KEY -e MCP_AUTH_TOKEN "lxagents-gitlab:$VERSION"
```

`GITLAB_API_KEY` is the GitLab token the server uses; `MCP_AUTH_TOKEN` is the access token, at least 32
characters. **A container is a server install:** with the access token set, the server inside it listens on every
interface on port 3000 by default, which is what lets a published port reach it, so `HOST` is not set. Without the
token it would listen on the container's own loopback, which `-p` cannot reach, and it says so at start; with
`HOST=0.0.0.0` and no token it refuses to start. A local install with no token is `MCP_TRANSPORT=http cargo run --release` on the
machine itself, not a container. `127.0.0.1:` in `-p` keeps the port reachable from this host only, and leaving it off publishes the port to the
network. Clients then connect to `http://127.0.0.1:3000/mcp` with `Authorization: Bearer <MCP_AUTH_TOKEN>`; see
[Set up HTTP with two tokens](../guides/setup-http-token.md). Put TLS in front before publishing the port beyond
the host. **This path, like the rest of the image, has not been run in a container.**

## What is in the image

| Path | Why |
|---|---|
| `/usr/local/bin/lxagents-gitlab` | The server, one static binary, built in the first stage. |
| `/etc/ssl/certs/` | Alpine's `ca-certificates`, which the TLS connection to GitLab is checked against. |

Only `Cargo.toml`, `Cargo.lock` and `src/` reach the build stage; nothing of the source reaches the image.

`.dockerignore` keeps the rest out: `test`, `wiki`, `.agents`, `.claude`, `AGENTS.md`, `README.md`,
`.git`, any `.env` file and local noise. **The image therefore cannot run its own test suite** —
`cargo test` needs `test/`, and the suite is a gate on the repository, not on the artifact.

## No `EXPOSE` and no compose file

There is no `EXPOSE`: the default is stdio, which has no port, and the HTTP transport is opt-in with
`MCP_TRANSPORT=http` and published with `-p`; see [Security model](../security/security-model.md). There
is no compose file either: a stdio server is not a service, and `docker compose up` would start it with
no stdin attached, so it would exit at once. A deployment — a registry, an orchestrator, a schedule — is
the owner's decision, not this repository's.

There is also no `ENTRYPOINT`. The whole default is `CMD ["lxagents-gitlab"]`, so a command after the image
name runs as written. An `ENTRYPOINT` of ours would turn `docker run <image> lxagents-gitlab --version` into
`lxagents-gitlab lxagents-gitlab --version`.

## Verifying an image

```bash
docker run --rm "lxagents-gitlab:$VERSION" lxagents-gitlab --version
```

Prints `lxagents-gitlab <version>` on stderr and exits 0. That shows the binary is on the path, runs
on the runtime base and reports its version, and it needs no token and no stdin. To check the tools, point
a client at the image as above and list them, or run `cargo test` in a checkout.

## Related pages

* [Local setup](setup.md) — running the server without a container.
* [Environment](env.md) — the variables.
* [Set up HTTP with two tokens](../guides/setup-http-token.md)
* [Connect a client](../guides/connect-a-client.md)
* [Security model](../security/security-model.md) — what a container does and does not change.
