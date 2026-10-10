/** List a project's tags. */
import { z } from "zod";
import { gitlab, pageQuery, paging, parseRepo } from "../api.js";
import { optionalText } from "../args.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../schemas.js";

export const TOOL = {
  name: "tag_get_all",
  description:
    "List the tags of a GitLab project, most recently updated first, with the commit each points at. " +
    "Paged. Use it to see what versions exist, or to find a tag before reading or releasing it.",
  inputSchema: {
    repo: repoArg,
    search: z.string().optional().describe("Only tags whose name contains this; '^v1' matches the start, 'rc$' the end."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(z.object({ name: z.string(), sha: z.string() })),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, search?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const search = optionalText(args.search, "search");
  const page = pageQuery(args);

  const { body, headers } = await gitlab("GET", `${repo.path}/repository/tags`, {
    query: { search, ...page },
    what: `list the tags of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((tag) => ({ name: tag.name, sha: tag.commit?.id ?? "" }));
  const matching = search === undefined ? "" : ` matching '${search}'`;
  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no tags${matching} on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "tag" : "tags"}${matching} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
