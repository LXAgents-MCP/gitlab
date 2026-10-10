#!/usr/bin/env node
/*
 * Server entry point.
 *
 * Gives an LLM access to GitLab over MCP, over stdio by default and over HTTP when
 * MCP_TRANSPORT=http. Two secrets, neither an argument:
 *
 * - GITLAB_API_KEY is the GitLab token, used for every call on both transports.
 * - MCP_AUTH_TOKEN is this server's own access token, which an HTTP client must present as
 *   `Authorization: Bearer <token>` (src/http.js). stdio never reads it.
 *
 * stdio: the client starts this process and speaks to it over a pipe; register it with the GitLab
 * token in its `env` block.
 *
 *   {"mcpServers": {"lxagents-gitlab": {
 *     "command": "node", "args": ["<this file>"],
 *     "env": {"GITLAB_API_KEY": "<token>"}}}}
 *
 * The key this server uses can change and delete projects. A pipe can only be reached by the
 * client that spawned it; the HTTP transport can be reached by anyone who can open its port, so it
 * listens on port 3000 on all interfaces unless PORT and HOST say otherwise, refuses to start without an
 * access token, and refuses any request that does not carry it. The only other network traffic is outbound, to gitlab.com,
 * and only when a tool is called.
 *
 * Nothing here may write to stdout: on stdio, stdout is the JSON-RPC channel.
 */
import { parseArgs } from "node:util";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { log } from "./log.js";
import { createServer } from "./server.js";
import { SERVER_NAME, VERSION } from "./version.js";

const USAGE = `LXAgents GitLab MCP Server

Usage: node src/index.js [options]

Serves GitLab tools over stdio, or over HTTP when MCP_TRANSPORT=http. Neither secret is
an argument:
  GITLAB_API_KEY  the GitLab token; set it in the environment of this process, read when a tool
                  is called. Used on both transports.
  MCP_AUTH_TOKEN  http only: the server's access token, at least 32 characters, which every
                  request must carry as Authorization: Bearer <token>. HTTP will not start
                  without it. stdio does not read it.
  PORT (default 3000) and HOST (default 0.0.0.0, every interface) say where HTTP listens.

Options:
  -h, --help       Show this help
  -v, --version    Show the version
`;

let values;
try {
  ({ values } = parseArgs({
    args: process.argv.slice(2),
    options: {
      help: { type: "boolean", short: "h" },
      version: { type: "boolean", short: "v" },
    },
    strict: true,
    allowPositionals: false,
  }));
} catch (error) {
  log(`${error.message}\n\n${USAGE}`);
  process.exit(2);
}

if (values.help) {
  log(USAGE);
  process.exit(0);
}
if (values.version) {
  log(`${SERVER_NAME} ${VERSION}`);
  process.exit(0);
}

// Which transport this process speaks, selected as `shared-instruction` selects it. stdio stays the
// default, because `node src/index.js` has to keep meaning what it meant.
const transportName = (process.env.MCP_TRANSPORT ?? "").trim().toLowerCase() || "stdio";

if (transportName === "http" || transportName === "streamable-http") {
  // Imported only now, so the stdio process never loads the HTTP code.
  const { main } = await import("./http.js");
  await main();
} else if (transportName === "stdio") {
  const server = createServer();
  await server.connect(new StdioServerTransport());

  // SIGTERM as well as SIGINT: in a container this process is PID 1, which the kernel gives no
  // default action for a signal, so without a handler `docker stop` waits out its grace period and
  // then kills it.
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, async () => {
      await server.close();
      process.exit(0);
    });
  }

  log(`${SERVER_NAME} ${VERSION} serving over stdio`);
} else {
  log(`MCP_TRANSPORT must be stdio or http, got '${transportName}'.`);
  process.exit(2);
}
