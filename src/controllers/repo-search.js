/** Search projects across GitLab. */
import { z } from "zod";
import { gitlab, pageQuery, paging } from "../services/api.js";
import { oneOf, requiredText } from "../models/args.js";
import { READ, pagingArgs, pagingOut } from "../models/schemas.js";

const SORTS = ["name", "path", "created_at", "updated_at", "last_activity_at", "star_count", "similarity"];
const ORDERS = ["asc", "desc"];

export const TOOL = {
  name: "repo_search",
  description:
    "Search GitLab projects by words in their name, path or description, such as 'mcp server'; several " +
    "words must all match. Use it to find a repository when its group is not known. It sees the " +
    "projects the token can see; similarity ranking only covers projects the token's user belongs to.",
  inputSchema: {
    query: z.string().describe("Words to look for in a project's name, path or description, e.g. 'mcp server'."),
    sort: z
      .enum(SORTS)
      .optional()
      .describe("name, path, created_at, updated_at, last_activity_at, star_count or similarity. Default: newest first."),
    order: z.enum(ORDERS).optional().describe("asc or desc (default). Only applies when sort is given."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(
      z.object({
        full_name: z.string(),
        description: z.string().optional(),
        stars: z.number(),
        last_activity_at: z.string().optional(),
        url: z.string(),
      }),
    ),
    total_count: z.number(),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ query?: unknown, sort?: unknown, order?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const query = requiredText(args.query, "query");
  const sort = oneOf(args.sort, "sort", SORTS);
  const order = oneOf(args.order, "order", ORDERS);
  const page = pageQuery(args);

  const { body, headers } = await gitlab("GET", "/projects", {
    query: { search: query, order_by: sort, sort: sort ? order : undefined, ...page },
    what: `search projects for '${query}'`,
  });

  const items = (Array.isArray(body) ? body : []).map((project) => ({
    full_name: project.path_with_namespace,
    ...(project.description ? { description: project.description } : {}),
    stars: project.star_count ?? 0,
    ...(project.last_activity_at ? { last_activity_at: project.last_activity_at } : {}),
    url: project.web_url,
  }));
  // GitLab leaves the total out of an answer with more than 10,000 matches.
  const counted = Number.parseInt(headers["x-total"] ?? "", 10);
  const total = Number.isInteger(counted) && counted >= 0 ? counted : items.length;

  return {
    payload: {
      summary:
        items.length === 0
          ? `No projects match '${query}'.`
          : `${total} ${total === 1 ? "project matches" : "projects match"} '${query}'; showing ${items.length}.`,
      items,
      total_count: total,
      ...paging(page, headers),
    },
    ok: true,
  };
}
