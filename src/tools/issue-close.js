/** Close an issue. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { rawText, requiredInt } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

export const TOOL = {
  name: "issue_close",
  description:
    "Close an issue of a GitLab project, optionally leaving a comment first. GitLab has no close reason, " +
    "so say why in `comment`. It only closes: there is no tool to reopen. Use pr_close for a merge request.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number in the project."),
    comment: z.string().optional().describe("A comment to post before closing, e.g. why."),
  },
  outputSchema: {
    number: z.number(),
    state: z.string(),
    html_url: z.string(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, comment?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const comment = rawText(args.comment, "comment");
  const commented = comment !== undefined && comment.trim() !== "";

  if (commented) {
    await gitlab("POST", `${repo.path}/issues/${number}/notes`, {
      json: { body: comment },
      what: `comment on issue #${number} of ${repo.full} before closing it`,
    });
  }

  let closed;
  try {
    closed = (
      await gitlab("PUT", `${repo.path}/issues/${number}`, {
        json: { state_event: "close" },
        what: `close issue #${number} of ${repo.full}`,
      })
    ).body;
  } catch (error) {
    if (error instanceof Refused && commented) throw new Refused(`${error.message} The comment had already been posted.`);
    throw error;
  }

  expectBody(closed, "web_url", "an issue");
  return {
    payload: {
      summary:
        closed.state === "closed"
          ? `Closed issue #${number} in ${repo.full}.`
          : `GitLab answered, but issue #${number} in ${repo.full} is still ${closed.state}.`,
      number,
      state: closed.state,
      html_url: closed.web_url,
    },
    ok: true,
  };
}
