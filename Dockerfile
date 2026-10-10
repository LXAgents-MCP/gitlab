# syntax=docker/dockerfile:1

# One stage. There is no build step, so a builder stage would copy the same files twice to
# produce a smaller context, not a smaller image.
FROM node:22-alpine

# Dependencies come from the lockfile only: `npm ci` fails rather than resolve something the
# lockfile does not contain, which is the point of building an image from a versioned tree.
# Lifecycle scripts are off, because running a dependency's install script by default is
# supply-chain risk with nothing to offset it, and `--omit=dev` keeps the test-only tree out.
# `.dockerignore` leaves the tests out too, so this image cannot run its own suite.
WORKDIR /srv

COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts --omit=dev

# `package.json` is read at start for the version, so it stays beside `src/`.
COPY src ./src

# Not root. The process speaks to GitLab and reads one file of its own; it needs nothing more.
USER node

# No EXPOSE, on purpose: this server has no listener and none should be added. A client reaches
# it through the container's stdin and stdout, so it runs with `docker run -i`. Without `-i`
# Docker attaches no stdin, the server sees it closed, and exits at once.
#
# The token is not here either. A value baked into an ENV line ships to everyone who pulls the
# image; it is passed at run time, `-e LXAGENTS_MCP_GITLAB_API_KEY`.
#
# The whole default is CMD and this file sets no ENTRYPOINT, so the base image's own
# `docker-entrypoint.sh` stays in place and a command after the image name runs as written. An
# ENTRYPOINT of ours would turn `docker run <image> node src/index.js --version` into
# `node src/index.js node src/index.js --version`.
CMD ["node", "src/index.js"]
