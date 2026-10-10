/** Comment on a merge request's discussion. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { rawText, requiredInt } from "../args.js";
import { WEB_BASE } from "../config.js";
import { Refused } from "../errors.js";
import { WRITE, repoArg } from "../schemas.js";

export const TOOL = {
  name: "pr_comment",
  description:
    "Add a comment to a merge request's discussion in a GitLab project. A general comment only: it is " +
    "not attached to a line of the diff and is not a review. Visible to everyone who can see the merge request.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Merge request number in the project."),
    body: z.string().describe("Comment text, in Markdown."),
  },
  outputSchema: {
    id: z.number(),
    html_url: z.string().optional(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, number?: unknown, body?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const body = rawText(args.body, "body", { required: true });

  const { body: created } = await gitlab("POST", `${repo.path}/merge_requests/${number}/notes`, {
    json: { body },
    what: `comment on merge request !${number} of ${repo.full}`,
  });

  if (!Number.isInteger(created?.id)) throw new Refused("GitLab answered, but the body was not a comment this server recognizes.");
  return {
    payload: {
      summary: `Commented on merge request !${number} in ${repo.full}.`,
      id: created.id,
      // The answer carries no address, so it is built from the path; a project given by id has none.
      ...(repo.full.includes("/") ? { html_url: `${WEB_BASE}/${repo.full}/-/merge_requests/${number}#note_${created.id}` } : {}),
    },
    ok: true,
  };
}
