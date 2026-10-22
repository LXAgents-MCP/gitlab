/** Ask people to review a merge request. */
import { z } from "zod";
import { gitlab, parseRepo } from "../services/api.js";
import { requiredInt, textList } from "../models/args.js";
import { UPDATE, repoArg } from "../models/schemas.js";
import { expectBody, namesIn } from "../models/shapes.js";
import { bareUsername, findUsers } from "../services/users.js";

export const TOOL = {
  name: "pr_review_request",
  description:
    "Request a review of a merge request in a GitLab project from people, who are added to those already " +
    "asked. An unknown username refuses the call before anything changes. GitLab has no team reviewers. " +
    "The result lists any person GitLab did not add.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Merge request number in the project."),
    reviewers: z.array(z.string()).describe("GitLab usernames to ask. At least one, at most 10."),
  },
  outputSchema: {
    number: z.number(),
    reviewers: z.array(z.string()),
    not_applied: z.array(z.string()).optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, reviewers?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const requested = textList(args.reviewers, "reviewers", { required: true, max: 10 })?.map(bareUsername) ?? [];
  const path = `${repo.path}/merge_requests/${number}`;
  const what = `request a review of merge request !${number} of ${repo.full}`;

  // GitLab replaces the whole list of reviewers, so the ones already asked are read and sent back with the new.
  const { body: before } = await gitlab("GET", path, { what: `read the reviewers of merge request !${number} of ${repo.full}` });
  expectBody(before, "web_url", "a merge request");
  const asked = (Array.isArray(before.reviewers) ? before.reviewers : []).filter((user) => Number.isInteger(user?.id));

  const people = await findUsers(requested, what);
  const next = [...new Set([...asked.map((user) => user.id), ...people.map((person) => person.id)])];

  let current = before;
  if (next.length !== asked.length) {
    ({ body: current } = await gitlab("PUT", path, { json: { reviewer_ids: next }, what }));
    expectBody(current, "web_url", "a merge request");
  }

  const now = namesIn(current.reviewers, "username");
  const notApplied = requested.filter((name) => !now.some((entry) => entry.toLowerCase() === name.toLowerCase()));
  return {
    payload: {
      summary:
        `Review requested on !${number} in ${repo.full} from: ${now.length > 0 ? now.join(", ") : "nobody"}.` +
        (notApplied.length > 0 ? ` GitLab did not add: ${notApplied.join(", ")}. They may not be able to review this project.` : ""),
      number,
      reviewers: now,
      ...(notApplied.length > 0 ? { not_applied: notApplied } : {}),
    },
    ok: true,
  };
}
