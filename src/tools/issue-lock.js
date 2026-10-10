/** Lock or unlock an issue's discussion. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { optionalFlag, requiredInt } from "../args.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

export const TOOL = {
  name: "issue_lock",
  description:
    "Lock the discussion of an issue in a GitLab project so only project members can comment, or unlock it. " +
    "GitLab takes no reason. Needs at least the Reporter role.",
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

/** @param {{ repo?: unknown, number?: unknown, locked?: unknown }} args */
export async function handle(args) {
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
