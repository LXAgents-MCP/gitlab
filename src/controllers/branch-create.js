/** Create a branch. */
import { z } from "zod";
import { gitlab, parseRepo, refName } from "../services/api.js";
import { Refused } from "../utils/errors.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";
import { short } from "../utils/text.js";

export const TOOL = {
  name: "branch_create",
  description:
    "Create a branch in a GitLab project, pointing at a branch, tag or commit. Use it to start " +
    "work off the default branch or off another branch. Fails if the name is taken.",
  inputSchema: {
    repo: repoArg,
    branch: z.string().describe("Name of the new branch, e.g. 'feat/login'."),
    from: z
      .string()
      .optional()
      .describe("Branch, tag or commit sha to start from. Default: the project's default branch."),
  },
  outputSchema: {
    branch: z.string(),
    sha: z.string(),
    from: z.string().optional(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, branch?: unknown, from?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const branch = refName(args.branch, "branch");
  const from = args.from === undefined || args.from === null ? undefined : refName(args.from, "from");

  // GitLab wants the starting point named, so the default branch is looked up when none is given.
  let ref = from;
  if (ref === undefined) {
    const { body } = await gitlab("GET", repo.path, { what: `find the default branch of ${repo.full}` });
    ref = body?.default_branch;
    if (typeof ref !== "string" || ref === "") {
      throw new Refused(
        `${repo.full} has no default branch, so there is nothing to branch from. Create its first commit with file_create, or pass \`from\`.`,
      );
    }
  }

  const { body } = await gitlab("POST", `${repo.path}/repository/branches`, {
    query: { branch, ref },
    what: `create the branch '${branch}' in ${repo.full}`,
    explain: (status, detail) => {
      if (status === 400 && /already exists/i.test(detail)) {
        return `GitLab could not create the branch '${branch}' (400)${detail}. A branch with that name already exists in ${repo.full}; pick another name.`;
      }
      if (status === 400 && /invalid reference|not found|does not exist/i.test(detail)) {
        return `GitLab could not create the branch '${branch}' (400)${detail}. '${ref}' is not a branch, tag or commit of ${repo.full}.`;
      }
      return undefined;
    },
  });

  expectBody(body, "name", "a branch");
  const sha = body.commit?.id ?? "";
  return {
    payload: {
      summary: `Created branch ${body.name} at ${short(sha)} in ${repo.full}, from ${ref}.`,
      branch: body.name,
      sha,
      from: ref,
    },
    ok: true,
  };
}
