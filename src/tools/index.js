/**
 * Tool registry: one module per tool, discovered rather than listed.
 *
 * Adding a tool is a new file in this directory that exports `TOOL` (name, description and
 * schemas) and `handle(args)`, resolving to `{ payload, ok }`. The file name is the tool name in
 * kebab case - `repo-create.js` is `repo_create` - and a test holds the two together in both
 * directions. Nothing needs editing elsewhere: `src/server.js` never names a tool, so the
 * protocol layer cannot grow a per-tool special case. A module that does not export both is
 * skipped, so a helper dropped here is never callable as a tool.
 */
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

/**
 * @typedef {object} ToolSpec
 * @property {string} name
 * @property {{ name: string, description: string, inputSchema: object, outputSchema: object, annotations: object }} TOOL
 * @property {(args: object) => Promise<{ payload: object, ok: boolean }>} handle
 */

/**
 * Import every tool module in `dir`.
 *
 * @param {string} [dir] Defaults to this directory; a different one is for tests.
 * @returns {Promise<Map<string, ToolSpec>>}
 */
export async function discoverTools(dir = HERE) {
  const found = new Map();
  const files = readdirSync(dir)
    .filter((file) => file.endsWith(".js") && file !== "index.js")
    .sort();

  for (const file of files) {
    const { TOOL, handle } = await import(pathToFileURL(join(dir, file)).href);
    if (TOOL === undefined || typeof handle !== "function") continue;

    if (!TOOL.name) throw new Error(`${file} TOOL has no 'name'.`);
    if (found.has(TOOL.name)) {
      // Two tools answering to one name makes tools/call dispatch arbitrary; fail loudly at
      // startup rather than silently at call time.
      throw new Error(`Duplicate tool name '${TOOL.name}' in ${file}.`);
    }
    found.set(TOOL.name, { name: TOOL.name, TOOL, handle });
  }

  return found;
}

export const REGISTRY = await discoverTools();

/** @type {ToolSpec[]} */
export const TOOLS = [...REGISTRY.values()];

/** @param {string} name */
export function get(name) {
  return REGISTRY.get(name);
}
