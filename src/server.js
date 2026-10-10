/**
 * The MCP server: protocol and tool dispatch only.
 *
 * A fresh instance is created per connection because McpServer holds per-connection state.
 * The tool surface is not written here - it is whatever `src/tools/` discovers.
 */
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { redact } from "./api.js";
import { tokenToScrub } from "./auth.js";
import { Refused } from "./errors.js";
import { log } from "./log.js";
import { renderPayload } from "./render.js";
import { TOOLS } from "./tools/index.js";
import { SERVER_NAME, VERSION } from "./version.js";

/** What every client reads at `initialize`, before it has called a single tool. */
const INSTRUCTIONS =
  "GitLab.com through its REST API: projects (called repositories here), branches, files, commits, tags, " +
  "releases, issues, merge requests (called pull requests here) and CI/CD pipelines (called workflows " +
  "here). Every tool needs GITLAB_API_KEY in this server's environment. Name a project by its " +
  "full path, 'group/project' or 'group/subgroup/project', or by its numeric id. Tools that delete or " +
  "merge are destructive: confirm with the owner before calling them.";

/**
 * Shape a handler's payload into a `tools/call` result.
 *
 * `isError` is how a client learns a call failed. It carries the message, so a failure arrives
 * as readable data rather than as an opaque protocol error. The text block is what the model
 * actually reads (see `render.js`); everything also rides along in `structuredContent` for a
 * client that would rather read it programmatically - except on a refusal or a crash, which
 * have no fields to report. A client validates `structuredContent` against the tool's output
 * schema whenever it is present, so an empty object there would be rejected for the fields it
 * lacks.
 *
 * @param {Record<string, unknown>} payload
 * @param {boolean} ok
 * @param {string} [message] The reason, for a refusal or a handler bug.
 */
export function toolResult(payload, ok, message = "") {
  const { summary = "", ...fields } = payload;
  const text = message ? `**${summary || "Refused"}**\n\n${message}` : renderPayload(summary, fields);

  const result = { content: [{ type: "text", text }], isError: !ok };
  if (!message) result.structuredContent = fields;
  return result;
}

/**
 * Run one tool call, turning every failure into a result the client can read.
 *
 * @param {import("./tools/index.js").ToolSpec} spec
 * @param {object} args
 */
async function callTool(spec, args) {
  try {
    const { payload, ok } = await spec.handle(args);
    return toolResult(payload, ok);
  } catch (error) {
    if (error instanceof Refused) {
      return toolResult({ summary: "Refused" }, false, error.message);
    }
    // A crash is text nobody wrote for a reader, so it is scrubbed like any other: the token
    // must not reach the log or the client whatever put it into an error message.
    const secret = tokenToScrub();
    log(redact(`Handler crashed:\n${error?.stack ?? error}`, secret));
    return toolResult(
      { summary: "Internal Error" },
      false,
      redact(`${error?.name ?? "Error"}: ${error?.message ?? error}`, secret),
    );
  }
}

/** @returns {McpServer} */
export function createServer() {
  const server = new McpServer({ name: SERVER_NAME, version: VERSION }, { instructions: INSTRUCTIONS });

  for (const spec of TOOLS) {
    const { description, inputSchema, outputSchema, annotations } = spec.TOOL;
    server.registerTool(
      spec.name,
      { description, inputSchema, outputSchema, annotations },
      (args) => callTool(spec, args),
    );
  }

  return server;
}
