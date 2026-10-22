/** Comment on an issue. */
import { z } from "zod";
import { gitlab, parseRepo } from "../services/api.js";
import { rawText, requiredInt } from "../models/args.js";
import { WEB_BASE } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { WRITE, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "issue_comment",
  description:
    "Add a comment to an issue of a GitLab project. Visible to everyone who can see the issue. On a " +
    "locked issue only project members can comment. For a merge request use pr_comment.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number in the project."),
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

  const { body: created } = await gitlab("POST", `${repo.path}/issues/${number}/notes`, {
    json: { body },
    what: `comment on issue #${number} of ${repo.full}`,
    explain: (status, detail) =>
      status === 403
        ? `GitLab would not let this comment on #${number} of ${repo.full} (403)${detail}. The token may lack the \`api\` ` +
          "scope, or the discussion may be locked, in which case only project members can comment."
        : undefined,
  });

  if (!Number.isInteger(created?.id)) throw new Refused("GitLab answered, but the body was not a comment this server recognizes.");
  return {
    payload: {
      summary: `Commented on #${number} in ${repo.full}.`,
      id: created.id,
      // The answer carries no address, so it is built from the path; a project given by id has none.
      ...(repo.full.includes("/") ? { html_url: `${WEB_BASE}/${repo.full}/-/issues/${number}#note_${created.id}` } : {}),
    },
    ok: true,
  };
}
