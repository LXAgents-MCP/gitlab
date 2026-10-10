# Docker

The server runs in a container over **stdio**, the only transport it has. The image is for a client
that can spawn `docker run -i`, and for a host that cannot run Node 20.

> **This image has not been built.** The environment this was written in had a Docker client and no
> daemon, so neither the `Dockerfile` nor the commands on this page have been run as written. What was
> checked instead: every `COPY` source exists and is not excluded by `.dockerignore`; the install step,
> `npm ci --ignore-scripts --omit=dev`, was run in a scratch directory holding only what the image
> copies, and the production-only tree it produced served every tool over a real stdio pipe; and
> `SIGTERM` is tested. Not checked: the base image, the `Dockerfile`'s syntax, and `USER node` in a real
> container. Build it once and run the check at the end of this page before relying on it.

## What a container is for here

* **A pinned toolchain.** `node:22-alpine` instead of whatever is on the host. For exact
  reproducibility, pin the base image by digest.
* **A clean dependency tree.** `npm ci --ignore-scripts --omit=dev` from the lockfile. `npm ci` fails
  rather than resolving something the lockfile does not contain.
* **A non-root process.** The image ends as `USER node`.
* **Isolation from the host.** The server reads no host file and runs no command either way; the
  container also keeps it off the host's file system altogether.

What it does not change: the token still reaches GitLab with whatever permissions it was granted, and
it is in the container's environment, where anyone who can `docker inspect` the container sees it.

## Build

```bash
VERSION=$(node -p "require('./package.json').version")
docker build -t "lxagents-gitlab:$VERSION" .
```

Tag it with the version in `package.json` rather than `latest`. The image's job is to be reproducible,
and `latest` is the one tag that cannot be.

## Run

```bash
docker run --rm -i -e LXAGENTS_MCP_GITLAB_API_KEY "lxagents-gitlab:$VERSION"
```

**`-i` is not optional.** Without it Docker attaches no stdin, the server sees it closed and exits
at once, which reads as a broken image rather than a missing flag. **Do not add `-t`:** a terminal
rewrites line endings on the protocol channel.

`-e LXAGENTS_MCP_GITLAB_API_KEY` with no value passes the variable through from the environment of the
`docker` process, so the token is not on the command line, in shell history or in the image. Never put
it in an `ENV` line: a value baked into the image ships to everyone who pulls it.

There is nothing to publish. The server has no port, and the image declares none.

## Register it with a client

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "-e", "LXAGENTS_MCP_GITLAB_API_KEY", "lxagents-gitlab:<version>"],
      "env": { "LXAGENTS_MCP_GITLAB_API_KEY": "<your GitLab token>" }
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

## What is in the image

| Path | Why |
|---|---|
| `src/` | The server. |
| `package.json`, `package-lock.json` | Dependency resolution, and the version the server reports. |
| `node_modules/` | The production tree: the MCP SDK and zod. Installed in the image, never copied in. |

`.dockerignore` keeps the rest out: `test`, `wiki`, `.agents`, `.claude`, `AGENTS.md`, `README.md`,
`.git`, any `.env` file and local noise. **The image therefore cannot run its own test suite** —
`npm test` needs `test/`, and the suite is a gate on the repository, not on the artifact.

## No port and no compose file

There is no `EXPOSE`, because there is no listener and none should be added; see
[Security model](../security/security-model.md). There is no compose file either: a stdio server is
not a service, and `docker compose up` would start it with no stdin attached, so it would exit at once.
A deployment — a registry, an orchestrator, a schedule — is the owner's decision, not this repository's.

There is also no `ENTRYPOINT`. The whole default is `CMD ["node", "src/index.js"]`, so the base image's
own entrypoint stays and a command after the image name runs as written. An `ENTRYPOINT` of ours would
turn `docker run <image> node src/index.js --version` into `node src/index.js node src/index.js --version`.

## Verifying an image

```bash
docker run --rm "lxagents-gitlab:$VERSION" node src/index.js --version
```

Prints `lxagents-gitlab <version>` on stderr and exits 0. That shows the entrypoint resolves, the
dependencies load and the version is read, and it needs no token and no stdin. To check the tools, point
a client at the image as above and list them, or run `npm test` in a checkout.

## Related pages

* [Local setup](setup.md) — running the server without a container.
* [Environment](env.md) — the one variable.
* [Connect a client](../guides/connect-a-client.md)
* [Security model](../security/security-model.md) — what a container does and does not change.
