#!/usr/bin/env node
/*
 * Server entry point.
 *
 * Gives an LLM access to GitLab over MCP. The client starts this process and speaks to it over
 * a pipe; register it in the client's configuration, with the token in its `env` block:
 *
 *   {"mcpServers": {"lxagents-gitlab": {
 *     "command": "node", "args": ["<this file>"],
 *     "env": {"LXAGENTS_MCP_GITLAB_API_KEY": "<token>"}}}}
 *
 * Stdio is the only transport, on purpose. The token this server holds can change and delete
 * projects, so there is no HTTP transport and no network listener: a process the client
 * spawns can only be reached by that client. The only network traffic is outbound, to
 * gitlab.com, and only when a tool is called.
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

Serves GitLab tools over stdio. Set LXAGENTS_MCP_GITLAB_API_KEY in the environment of this
process; it is read when a tool is called, never taken as an argument.

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
