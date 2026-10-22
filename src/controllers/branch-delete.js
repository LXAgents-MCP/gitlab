/** Delete a branch. */
import { z } from "zod";
import { gitlab, parseRepo, refName, refPath } from "../services/api.js";
import { DESTROY, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "branch_delete",
  description:
    "Delete a branch from a GitLab project. Destructive: commits only that branch holds become " +
    "unreachable, so confirm with the owner first. The default branch and protected branches cannot " +
    "be deleted.",
  inputSchema: {
    repo: repoArg,
    branch: z.string().describe("Name of the branch to delete."),
  },
  outputSchema: {
    deleted: z.boolean(),
    branch: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, branch?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const branch = refName(args.branch, "branch");

  await gitlab("DELETE", `${repo.path}/repository/branches/${refPath(branch)}`, {
    what: `delete the branch '${branch}' from ${repo.full}`,
    explain: (status, detail) =>
      status === 400 || status === 403 || status === 404 || status === 422
        ? `GitLab would not delete the branch '${branch}' from ${repo.full} (${status})${detail}. The branch ` +
          "may not exist, or it is the default branch or a protected branch, which cannot be deleted; " +
          "deleting needs the Developer role or more."
        : undefined,
  });

  return {
    payload: { summary: `Deleted branch ${branch} from ${repo.full}.`, deleted: true, branch },
    ok: true,
  };
}
