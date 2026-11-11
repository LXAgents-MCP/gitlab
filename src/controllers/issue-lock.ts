/** Lock or unlock an issue's discussion. */
import { z } from "zod";
import { gitlab } from "../services/api.js";
import { parseRepo } from "../models/address.js";
import { optionalFlag, requiredInt } from "../models/args.js";
import { UPDATE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";
import type { ToolDefinition, ToolOutcome } from "./index.js";

export const TOOL: ToolDefinition = {
  name: "issue_lock",
  description:
    "Lock the discussion of an issue in a GitLab project so only project members can comment, or unlock it. " +
    "GitLab takes no reason. Needs permission to edit the issue.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number in the project."),
    locked: z.boolean().optional().describe("true locks (default), false unlocks."),
  },
  outputSchema: {
    number: z.number(),
    locked: z.boolean(),
  },
  annotations: UPDATE,
};

export async function handle(args: { repo?: unknown; number?: unknown; locked?: unknown }): Promise<ToolOutcome> {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const lock = optionalFlag(args.locked, "locked", true);

  const { body } = await gitlab("PUT", `${repo.path}/issues/${number}`, {
    json: { discussion_locked: lock },
    what: `${lock ? "lock" : "unlock"} #${number} of ${repo.full}`,
  });

  expectBody(body, "web_url", "an issue");
  const locked = Boolean(body.discussion_locked);
  return {
    payload: {
      summary:
        locked === lock
          ? `${lock ? "Locked" : "Unlocked"} #${number} in ${repo.full}.`
          : `GitLab answered, but #${number} in ${repo.full} is still ${locked ? "locked" : "unlocked"}.`,
      number,
      locked,
    },
    ok: true,
  };
}
