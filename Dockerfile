# syntax=docker/dockerfile:1

# Two stages. The server is one Rust binary, so the first stage compiles it and the second holds only
# that binary on a bare Alpine. Neither the sources nor the toolchain reach the image.
FROM rust:1.97-alpine AS build

# The TLS stack (rustls over aws-lc) is C and assembly underneath, so its build needs a C toolchain,
# cmake and perl. musl links the binary statically, which is what lets the runtime stage be bare.
RUN apk add --no-cache build-base cmake perl clang

# Dependencies come from the lockfile only: `--locked` fails rather than resolve something the lockfile
# does not contain, which is the point of building an image from a versioned tree.
WORKDIR /srv
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bin lxagents-gitlab

FROM alpine:3.22

# The trust store for the TLS connection to GitLab, and nothing else.
RUN apk add --no-cache ca-certificates
COPY --from=build /srv/target/release/lxagents-gitlab /usr/local/bin/lxagents-gitlab

# Not root. The process speaks to GitLab and reads nothing of its own; it needs nothing more.
USER nobody

# No EXPOSE, on purpose: the default is stdio, which has no port, and HTTP is opt-in with
# `-e MCP_TRANSPORT=http`, published by `-p` at run time. A stdio client reaches it through the
# container's stdin and stdout, so it runs with `docker run -i`. Without `-i` Docker attaches no stdin,
# the server sees it closed, and exits at once.
#
# No secret is here either. A value baked into an ENV line ships to everyone who pulls the image; both
# are passed at run time: `-e GITLAB_API_KEY` for the GitLab key, and for HTTP also `-e MCP_AUTH_TOKEN`,
# the access token clients present.
#
# The whole default is CMD and this file sets no ENTRYPOINT, so a command after the image name runs as
# written: `docker run <image> lxagents-gitlab --version`.
CMD ["lxagents-gitlab"]
