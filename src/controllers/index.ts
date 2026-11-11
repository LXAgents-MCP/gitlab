/**
 * Tool registry: one module per tool, discovered rather than listed.
 *
 * Adding a tool is a new file in this directory that exports `TOOL` (name, description and
 * schemas) and `handle(args)`, resolving to `{ payload, ok }`. The file name is the tool name in
 * kebab case - `repo-create.ts` is `repo_create` - and a test holds the two together in both
 * directions. Nothing needs editing elsewhere: `src/routes/server.ts` never names a tool, so the
 * protocol layer cannot grow a per-tool special case. A module that does not export both is
 * skipped, so a helper dropped here is never callable as a tool.
 */
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import type { ZodRawShape } from "zod";
import type { ToolAnnotations } from "@modelcontextprotocol/sdk/types.js";

const HERE = dirname(fileURLToPath(import.meta.url));

/** What a tool module declares: its name, what it is for, its schemas and its hints. */
export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: ZodRawShape;
  outputSchema: ZodRawShape;
  annotations: ToolAnnotations;
}

/** What a tool returns: a one-sentence `summary` plus the fields its output schema names. */
export interface ToolOutcome {
  payload: { summary: string } & Record<string, unknown>;
  ok: boolean;
}

/** A tool module's handler. Its arguments are checked by the handler itself, so they arrive unknown. */
export type ToolHandler = (args: Record<string, unknown>) => Promise<ToolOutcome>;

/** A tool as the registry holds it. */
export interface ToolSpec {
  name: string;
  TOOL: ToolDefinition;
  handle: ToolHandler;
}

/**
 * Whether a file in this directory is a tool module: `.ts` when the sources run through tsx, `.js`
 * when the compiled server runs from `dist/`. A declaration file or a source map is neither.
 */
function isToolModule(file: string): boolean {
  if (file.endsWith(".d.ts")) return false;
  if (!file.endsWith(".js") && !file.endsWith(".ts")) return false;
  return file !== "index.js" && file !== "index.ts";
}

/**
 * Import every tool module in `dir`.
 *
 * @param dir Defaults to this directory; a different one is for tests.
 */
export async function discoverTools(dir: string = HERE): Promise<Map<string, ToolSpec>> {
  const found = new Map<string, ToolSpec>();
  const files = readdirSync(dir).filter(isToolModule).sort();

  for (const file of files) {
    const { TOOL, handle } = (await import(pathToFileURL(join(dir, file)).href)) as {
      TOOL?: ToolDefinition;
      handle?: ToolHandler;
    };
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

export const TOOLS: ToolSpec[] = [...REGISTRY.values()];

export function get(name: string): ToolSpec | undefined {
  return REGISTRY.get(name);
}
