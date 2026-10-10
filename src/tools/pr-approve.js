/** Approve a merge request. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { optionalText, requiredInt } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";

export const TOOL = {
  name: "pr_approve",
  description:
    "Approve a merge request in a GitLab project as the token's user. GitLab approvals take no message. " +
    "Whether a person may approve their own merge request, and whether an approval is needed to merge at all, " +
    "are the project's settings. Approving again changes nothing more.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Merge request number in the project."),
    sha: z
      .string()
      .optional()
      .describe("The head commit being approved; the approval fails if the branch has moved on. Default: the latest."),
  },
  outputSchema: {
    number: z.number(),
    approved_by: z.array(z.string()),
    approvals_left: z.number().optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, sha?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const sha = optionalText(args.sha, "sha");

  const { body } = await gitlab("POST", `${repo.path}/merge_requests/${number}/approve`, {
    ...(sha === undefined ? {} : { json: { sha } }),
    what: `approve merge request !${number} of ${repo.full}`,
    explain: (status, detail) =>
      status === 401
        ? `GitLab would not record the approval on !${number} of ${repo.full} (401)${detail}. The token's user may not ` +
          "approve this merge request: the project may forbid approving your own, or the user lacks the role, " +
          "or the token is not valid."
        : status === 409
          ? `GitLab did not record the approval on !${number} of ${repo.full} (409)${detail}. The branch moved on from ` +
            "the commit that was meant to be approved; read the merge request again with pr_read."
          : undefined,
  });

  if (!body || typeof body !== "object" || !Array.isArray(body.approved_by)) {
    throw new Refused("GitLab answered, but the body was not an approval this server recognizes.");
  }
  const approvedBy = body.approved_by.map((entry) => entry?.user?.username).filter((name) => typeof name === "string");
  const left = Number.isInteger(body.approvals_left) ? body.approvals_left : undefined;
  return {
    payload: {
      summary:
        `Approved merge request !${number} in ${repo.full}; approved by ${approvedBy.length > 0 ? approvedBy.join(", ") : "nobody listed"}` +
        (left === undefined ? "." : left === 0 ? ", no approvals left to wait for." : `, ${left} more needed.`),
      number,
      approved_by: approvedBy,
      ...(left === undefined ? {} : { approvals_left: left }),
    },
    ok: true,
  };
}
