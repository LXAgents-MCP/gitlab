/** List a project's releases. */
import { z } from "zod";
import { gitlab, pageQuery, paging, parseRepo } from "../services/api.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "release_list",
  description:
    "List the releases of a GitLab project, newest release date first, upcoming ones included. Paged. " +
    "Use release_read for one release's notes and assets.",
  inputSchema: { repo: repoArg, ...pagingArgs },
  outputSchema: {
    items: z.array(
      z.object({
        tag: z.string(),
        name: z.string().optional(),
        upcoming: z.boolean(),
        released_at: z.string().optional(),
      }),
    ),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const page = pageQuery(args);

  const { body, headers } = await gitlab("GET", `${repo.path}/releases`, {
    query: { order_by: "released_at", sort: "desc", ...page },
    what: `list the releases of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((release) => ({
    tag: release.tag_name,
    ...(release.name ? { name: release.name } : {}),
    upcoming: Boolean(release.upcoming_release),
    ...(release.released_at ? { released_at: release.released_at } : {}),
  }));

  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no releases on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "release" : "releases"} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
