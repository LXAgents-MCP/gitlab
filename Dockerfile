# syntax=docker/dockerfile:1

# Two stages. The server is TypeScript compiled to `dist/` by `tsc`, which is a dev dependency, so the
# first stage installs everything and builds, and the second holds only the production tree and the
# compiled output. Neither the sources nor the compiler reach the image.
FROM node:22-alpine AS build

# Dependencies come from the lockfile only: `npm ci` fails rather than resolve something the
# lockfile does not contain, which is the point of building an image from a versioned tree.
# Lifecycle scripts are off, because running a dependency's install script by default is
# supply-chain risk with nothing to offset it.
WORKDIR /srv
COPY package.json package-lock.json tsconfig.json ./
RUN npm ci --ignore-scripts
COPY src ./src
RUN npm run build

FROM node:22-alpine

# `--omit=dev` keeps the compiler and the test-only tree out. `.dockerignore` leaves the tests out
# too, so this image cannot run its own suite.
WORKDIR /srv

COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts --omit=dev

# `package.json` is read at start for the version, so it stays beside `dist/`.
COPY --from=build /srv/dist ./dist

# Not root. The process speaks to GitLab and reads one file of its own; it needs nothing more.
USER node

# No EXPOSE, on purpose: the default is stdio, which has no port, and HTTP is opt-in with
# `-e MCP_TRANSPORT=http`, published by `-p` at run time. A stdio client reaches
# it through the container's stdin and stdout, so it runs with `docker run -i`. Without `-i`
# Docker attaches no stdin, the server sees it closed, and exits at once.
#
# No secret is here either. A value baked into an ENV line ships to everyone who pulls the
# image; both are passed at run time: `-e GITLAB_API_KEY` for the GitLab key, and for HTTP also
# `-e MCP_AUTH_TOKEN`, the access token clients present.
#
# The whole default is CMD and this file sets no ENTRYPOINT, so the base image's own
# `docker-entrypoint.sh` stays in place and a command after the image name runs as written. An
# ENTRYPOINT of ours would turn `docker run <image> node dist/index.js --version` into
# `node dist/index.js node dist/index.js --version`.
CMD ["node", "dist/index.js"]
