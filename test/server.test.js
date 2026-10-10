import assert from "node:assert/strict";
import test from "node:test";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { createServer, toolResult } from "../src/server.js";
import { get } from "../src/tools/index.js";
import { SERVER_NAME, VERSION } from "../src/version.js";
import { KIND_HINTS, SURFACE, TOOL_NAMES } from "./surface.js";
import { TOKEN, mockApi, reply, withToken, withoutToken } from "./helpers.js";

/** A client connected to a fresh server over an in-memory pipe. */
async function connect(t) {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "test", version: "0.0.0" });
  const server = createServer();
  await Promise.all([server.connect(serverTransport), client.connect(clientTransport)]);
  t.after(async () => {
    await client.close();
    await server.close();
  });
  return client;
}

const text = (result) => {
  assert.equal(result.content.length, 1, "expected exactly one content block");
  assert.equal(result.content[0].type, "text");
  return result.content[0].text;
};

test("the server identifies itself, offers tools only, and tells a client how to use it", async (t) => {
  const client = await connect(t);

  assert.deepEqual(client.getServerVersion(), { name: SERVER_NAME, version: VERSION });
  assert.deepEqual(Object.keys(client.getServerCapabilities()), ["tools"]);
  assert.match(client.getInstructions(), /GITLAB_API_KEY/);
  assert.match(client.getInstructions(), /'group\/project'/);
});

test("tools/list advertises exactly the surface, with schemas and hints", async (t) => {
  const client = await connect(t);
  const { tools } = await client.listTools();

  assert.deepEqual(tools.map((tool) => tool.name).sort(), TOOL_NAMES);
  for (const tool of tools) {
    assert.equal(tool.outputSchema.type, "object", `${tool.name} has no output schema`);
    assert.ok(tool.description.length > 20);
    assert.deepEqual(tool.annotations, KIND_HINTS[SURFACE[tool.name]], tool.name);
  }

  const byName = Object.fromEntries(tools.map((tool) => [tool.name, tool]));
  assert.deepEqual(byName.repo_create.inputSchema.required, ["name"]);
});

test("repo_create over the protocol puts the repository in the text and in the structured content", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(201, {
      id: 7,
      name: "demo",
      path: "demo",
      path_with_namespace: "acme/demo",
      web_url: "https://gitlab.com/acme/demo",
      http_url_to_repo: "https://gitlab.com/acme/demo.git",
      visibility: "private",
      default_branch: "main",
    }),
  );
  const client = await connect(t);

  const result = await client.callTool({ name: "repo_create", arguments: { name: "demo" } });

  assert.equal(result.isError, false);
  assert.equal(
    text(result),
    "**Created private repository acme/demo on GitLab.**\n\n" +
      "- name: demo\n" +
      "- full_name: acme/demo\n" +
      "- html_url: https://gitlab.com/acme/demo\n" +
      "- clone_url: https://gitlab.com/acme/demo.git\n" +
      "- visibility: private\n" +
      "- default_branch: main",
  );
  assert.equal(result.structuredContent.html_url, "https://gitlab.com/acme/demo");
  assert.equal("summary" in result.structuredContent, false);
});

test("a refusal is an error result with the reason, and no structured content", async (t) => {
  withoutToken(t);
  const client = await connect(t);

  // The client throws on structuredContent that does not match the output schema, so this
  // resolving is also the check that a refusal does not send an empty one.
  const result = await client.callTool({ name: "repo_create", arguments: { name: "demo" } });

  assert.equal(result.isError, true);
  assert.equal(result.structuredContent, undefined);
  assert.match(text(result), /^\*\*Refused\*\*\n\nGITLAB_API_KEY is not set\./);
});

test("a call with a missing argument, or to no such tool, is an error result and not a crash", async (t) => {
  const client = await connect(t);

  const missing = await client.callTool({ name: "repo_create", arguments: {} });
  assert.equal(missing.isError, true);
  assert.match(text(missing), /name/);

  const unknown = await client.callTool({ name: "no_such_tool", arguments: {} });
  assert.equal(unknown.isError, true);
  assert.match(text(unknown), /not found/i);

  // The server is still serving.
  const { tools } = await client.listTools();
  assert.equal(tools.length, TOOL_NAMES.length);
});

test("a handler that crashes is reported as an internal error and the server carries on", async (t) => {
  withoutToken(t);
  const client = await connect(t);
  const spec = get("repo_create");
  const original = spec.handle;
  spec.handle = async () => {
    throw new TypeError("boom");
  };
  t.after(() => {
    spec.handle = original;
  });
  const written = t.mock.method(process.stderr, "write", () => true);

  const result = await client.callTool({ name: "repo_create", arguments: { name: "demo" } });

  assert.equal(result.isError, true);
  assert.equal(result.structuredContent, undefined);
  assert.equal(text(result), "**Internal Error**\n\nTypeError: boom");
  assert.match(written.mock.calls[0].arguments[0], /^Handler crashed:\nTypeError: boom/);

  spec.handle = original;
  const after = await client.callTool({ name: "repo_create", arguments: { name: "demo" } });
  assert.match(text(after), /^\*\*Refused\*\*/);
});

test("a crash never carries the token, in the result or in the log", async (t) => {
  withToken(t);
  const client = await connect(t);
  const spec = get("repo_create");
  const original = spec.handle;
  spec.handle = async () => {
    throw new Error(`upstream said ${TOKEN}`);
  };
  t.after(() => {
    spec.handle = original;
  });
  const written = t.mock.method(process.stderr, "write", () => true);

  const result = await client.callTool({ name: "repo_create", arguments: { name: "demo" } });

  assert.equal(result.isError, true);
  assert.equal(text(result), "**Internal Error**\n\nError: upstream said ***");
  assert.ok(written.mock.calls.every((call) => !String(call.arguments[0]).includes(TOKEN)));
  assert.match(written.mock.calls[0].arguments[0], /upstream said \*\*\*/);
});

test("toolResult renders a success and a refusal", () => {
  const ok = toolResult({ summary: "Done.", id: 7, text: "body" }, true);
  assert.equal(ok.isError, false);
  assert.equal(ok.content[0].text, "**Done.**\n\n- id: 7\n\n```text\nbody\n```");
  assert.deepEqual(ok.structuredContent, { id: 7, text: "body" });

  const refused = toolResult({ summary: "Boom" }, false, "why");
  assert.equal(refused.content[0].text, "**Boom**\n\nwhy");
  assert.equal(refused.structuredContent, undefined);

  assert.equal(toolResult({}, false, "why").content[0].text, "**Refused**\n\nwhy");
});
