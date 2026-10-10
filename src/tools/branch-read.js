/** Read one branch. */
import { z } from "zod";
import { gitlab, parseRepo, refName, refPath } from "../api.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";
import { firstLine, short } from "../text.js";

export const TOOL = {
  name: "branch_read",
  description:
    "Read one branch of a GitLab project: the commit at its tip, that commit's title and date, whether " +
    "the branch is protected and whether it is the default. Use it to find a branch's current sha.",
  inputSchema: {
    repo: repoArg,
    branch: z.string().describe("Branch name, e.g. 'main' or 'feat/login'."),
  },
  outputSchema: {
    name: z.string(),
    sha: z.string(),
    protected: z.boolean(),
    default: z.boolean(),
    message: z.string().optional(),
    date: z.string().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, branch?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const branch = refName(args.branch, "branch");

  const { body } = await gitlab("GET", `${repo.path}/repository/branches/${refPath(branch)}`, {
    what: `read the branch '${branch}' of ${repo.full}`,
  });

  expectBody(body, "name", "a branch");
  const sha = body.commit?.id ?? "";
  const message = body.commit?.title ?? firstLine(body.commit?.message);
  const date = body.commit?.committed_date;
  return {
    payload: {
      summary: `Branch ${body.name} of ${repo.full} is at ${short(sha)}${body.default ? ", the default" : ""}${body.protected ? " and is protected" : ""}.`,
      name: body.name,
      sha,
      protected: Boolean(body.protected),
      default: Boolean(body.default),
      ...(message ? { message } : {}),
      ...(date ? { date } : {}),
    },
    ok: true,
  };
}
