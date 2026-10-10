/** Close a merge request without merging it. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { rawText, requiredInt } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

export const TOOL = {
  name: "pr_close",
  description:
    "Close a merge request in a GitLab project without merging it, optionally leaving a comment first. " +
    "Its branch is kept. A merge request that is already merged cannot be closed. There is no tool to reopen it.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Merge request number in the project."),
    comment: z.string().optional().describe("A comment to post before closing, e.g. why."),
  },
  outputSchema: {
    number: z.number(),
    state: z.string(),
    merged: z.boolean(),
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
  const path = `${repo.path}/merge_requests/${number}`;

  if (commented) {
    await gitlab("POST", `${path}/notes`, {
      json: { body: comment },
      what: `comment on merge request !${number} of ${repo.full} before closing it`,
    });
  }

  let closed;
  try {
    closed = (
      await gitlab("PUT", path, {
        json: { state_event: "close" },
        what: `close merge request !${number} of ${repo.full}`,
        explain: (status, detail) =>
          status === 405 || status === 422
            ? `GitLab would not close merge request !${number} of ${repo.full} (${status})${detail}. A merge request that ` +
              "is already merged cannot be closed; read it with pr_read."
            : undefined,
      })
    ).body;
  } catch (error) {
    if (error instanceof Refused && commented) throw new Refused(`${error.message} The comment had already been posted.`);
    throw error;
  }

  expectBody(closed, "web_url", "a merge request");
  const merged = closed.state === "merged";
  return {
    payload: {
      summary:
        closed.state === "closed"
          ? `Closed merge request !${number} in ${repo.full} without merging.`
          : `GitLab answered, but merge request !${number} in ${repo.full} is ${closed.state}.`,
      number,
      state: closed.state,
      merged,
      html_url: closed.web_url,
    },
    ok: true,
  };
}
