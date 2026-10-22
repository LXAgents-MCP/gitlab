import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { REGISTRY, TOOLS, discoverTools, get } from "../src/controllers/index.js";
import { KIND_HINTS, NO_REPO_ARGUMENT, SURFACE, TOOL_NAMES } from "./surface.js";

const TOOLS_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "controllers");

test("the tools discovered are exactly the surface this server is meant to serve", () => {
  assert.deepEqual(TOOLS.map((tool) => tool.name).sort(), TOOL_NAMES);
  assert.equal(get("repo_create").name, "repo_create");
  assert.equal(get("nope"), undefined);
});

test("every tool module is a tool and every tool is a module, named alike", () => {
  // Everything in the directory but the registry is a tool, and its file name is its tool name
  // in kebab case. Both directions: a stray file fails, and so does a tool in the wrong file.
  const files = readdirSync(TOOLS_DIR)
    .filter((file) => file.endsWith(".js") && file !== "index.js")
    .map((file) => file.replace(/\.js$/, "").replaceAll("-", "_"))
    .sort();

  assert.deepEqual([...REGISTRY.keys()].sort(), files);
});

test("every tool carries a description, an output schema and annotations", () => {
  for (const { TOOL } of TOOLS) {
    assert.ok(TOOL.description.length > 20, `${TOOL.name} has no useful description`);
    assert.ok(TOOL.description.length <= 600, `${TOOL.name} description is too long to sit in context 45 times`);
    assert.ok(Object.keys(TOOL.outputSchema).length > 0, `${TOOL.name} has no output schema`);
    for (const hint of ["readOnlyHint", "destructiveHint", "idempotentHint", "openWorldHint"]) {
      assert.equal(typeof TOOL.annotations[hint], "boolean", `${TOOL.name} lacks ${hint}`);
    }
  }
});

test("every tool's annotations match the kind of change it makes", () => {
  for (const { TOOL } of TOOLS) {
    const kind = SURFACE[TOOL.name];
    assert.ok(kind, `${TOOL.name} is not tagged in test/surface.js`);
    assert.deepEqual(TOOL.annotations, KIND_HINTS[kind], `${TOOL.name} is a ${kind} but is annotated otherwise`);
  }
});

test("every tool but the two that cannot takes a required repo", () => {
  for (const { TOOL } of TOOLS) {
    if (NO_REPO_ARGUMENT.includes(TOOL.name)) {
      assert.equal("repo" in TOOL.inputSchema, false, `${TOOL.name} should not take a repo`);
    } else {
      assert.ok("repo" in TOOL.inputSchema, `${TOOL.name} takes no repo`);
      assert.equal(TOOL.inputSchema.repo.isOptional(), false, `${TOOL.name} makes repo optional`);
    }
  }
});

test("no tool takes a credential as an argument", () => {
  for (const { TOOL } of TOOLS) {
    for (const key of Object.keys(TOOL.inputSchema)) {
      assert.doesNotMatch(key, /token|secret|password|api_?key|authorization|credential/i, `${TOOL.name}.${key}`);
    }
  }
});

/** A directory of tool-shaped fixture modules, removed when the test ends. */
function fixtures(t, files) {
  const dir = mkdtempSync(join(tmpdir(), "gitlab-registry-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [name, source] of Object.entries(files)) writeFileSync(join(dir, name), source);
  return dir;
}

const tool = (name) =>
  `export const TOOL = { name: ${JSON.stringify(name)} };\nexport async function handle() {}\n`;

test("discovery skips a module that is not a tool", async (t) => {
  const dir = fixtures(t, { "a.js": tool("alpha"), "helper.js": "export const value = 1;\n" });

  const found = await discoverTools(dir);
  assert.deepEqual([...found.keys()], ["alpha"]);
});

test("discovery refuses two tools with one name", async (t) => {
  const dir = fixtures(t, { "a.js": tool("same"), "b.js": tool("same") });

  await assert.rejects(discoverTools(dir), /Duplicate tool name 'same' in b\.js/);
});

test("discovery refuses a tool with no name", async (t) => {
  const dir = fixtures(t, { "a.js": tool("") });

  await assert.rejects(discoverTools(dir), /a\.js TOOL has no 'name'/);
});
