import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { SERVER_NAME, VERSION } from "../src/version.js";
import { TOOL_NAMES } from "./surface.js";

/*
 * The server as a client actually runs it: a real process, spoken to over a real pipe. The
 * things that go wrong here - a stray write to stdout, a process that outlives its client - do
 * not reproduce on an in-memory transport.
 */

const ENTRY = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "index.js");

/** A client attached to `node src/index.js <args>` over stdio, with no token in its environment. */
async function connect(t, args = []) {
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [ENTRY, ...args],
    stderr: "pipe",
  });
  const client = new Client({ name: "test", version: "0.0.0" });
  await client.connect(transport);
  t.after(() => client.close());
  return client;
}

test("over stdio the server lists exactly its surface and answers a call", async (t) => {
  const client = await connect(t);

  const { tools } = await client.listTools();
  assert.deepEqual(tools.map((tool) => tool.name).sort(), TOOL_NAMES);

  // The spawned process inherits no token, so the call is refused - which is the answer being
  // checked: a refusal that names the variable, not a crash and not a hang.
  const result = await client.callTool({ name: "repo_create", arguments: { name: "demo" } });
  assert.equal(result.isError, true);
  assert.match(result.content[0].text, /MCP_AUTH_TOKEN is not set/);
});

test("a bad command line exits 2 with the reason on stderr and nothing on stdout", () => {
  for (const args of [["--bogus"], ["stray"], ["--help=1"]]) {
    const run = spawnSync(process.execPath, [ENTRY, ...args], { encoding: "utf8" });

    assert.equal(run.status, 2, args.join(" "));
    assert.equal(run.stdout, "", `${args.join(" ")} wrote to stdout`);
    assert.match(run.stderr, /Usage: node src\/index\.js/);
  }
});

test("--help prints the usage to stderr, leaves stdout empty and exits 0", () => {
  const run = spawnSync(process.execPath, [ENTRY, "--help"], { encoding: "utf8" });

  assert.equal(run.status, 0);
  assert.equal(run.stdout, "");
  assert.match(run.stderr, /MCP_AUTH_TOKEN/);
});

test("--version prints the name and version to stderr, leaves stdout empty and exits 0", () => {
  const run = spawnSync(process.execPath, [ENTRY, "--version"], { encoding: "utf8" });

  assert.equal(run.status, 0);
  assert.equal(run.stdout, "");
  assert.equal(run.stderr.trim(), `${SERVER_NAME} ${VERSION}`);
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  test(`${signal} closes the server and exits 0, which a container's first process needs`, async () => {
    const child = spawn(process.execPath, [ENTRY], { stdio: ["pipe", "pipe", "pipe"] });
    const exited = new Promise((resolve) => child.on("exit", (code, by) => resolve({ code, by })));
    // The handlers are installed before this line is written, so it is safe to signal after it.
    await new Promise((resolve) => {
      child.stderr.on("data", (chunk) => String(chunk).includes("serving over stdio") && resolve());
    });

    child.kill(signal);

    const result = await Promise.race([
      exited,
      new Promise((resolve) => setTimeout(() => resolve("still running"), 5000)),
    ]);
    if (result === "still running") child.kill("SIGKILL");
    assert.deepEqual(result, { code: 0, by: null });
  });
}

test("the process exits by itself when its client goes away", async () => {
  const child = spawn(process.execPath, [ENTRY], { stdio: ["pipe", "pipe", "ignore"] });
  const exited = new Promise((resolve) => child.on("exit", (code) => resolve(code)));

  child.stdin.end();

  const code = await Promise.race([
    exited,
    new Promise((resolve) => setTimeout(() => resolve("still running"), 5000)),
  ]);
  if (code === "still running") child.kill("SIGKILL");
  assert.equal(code, 0);
});
