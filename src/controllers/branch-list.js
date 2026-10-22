/** List a project's branches. */
import { z } from "zod";
import { gitlab, pageQuery, paging, parseRepo } from "../services/api.js";
import { optionalText } from "../models/args.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "branch_list",
  description:
    "List the branches of a GitLab project by name, with the sha at each tip and whether it is " +
    "protected, the default or already merged. Paged. Use it to see what branches exist before " +
    "creating, merging or deleting one.",
  inputSchema: {
    repo: repoArg,
    search: z
      .string()
      .optional()
      .describe("Only branches whose name contains this; '^feat' matches the start, 'fix$' the end."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(
      z.object({ name: z.string(), sha: z.string(), protected: z.boolean(), default: z.boolean(), merged: z.boolean() }),
    ),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, search?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const search = optionalText(args.search, "search");
  const page = pageQuery(args);

  const { body, headers } = await gitlab("GET", `${repo.path}/repository/branches`, {
    query: { search, ...page },
    what: `list the branches of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((branch) => ({
    name: branch.name,
    sha: branch.commit?.id ?? "",
    protected: Boolean(branch.protected),
    default: Boolean(branch.default),
    merged: Boolean(branch.merged),
  }));
  const matching = search === undefined ? "" : ` matching '${search}'`;

  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no branches${matching} on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "branch" : "branches"}${matching} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
