/** List the commits of a branch. */
import { z } from "zod";
import { filePath, gitlab, pageQuery, paging, parseRepo, refName } from "../services/api.js";
import { optionalDate, optionalText } from "../models/args.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

export const TOOL = {
  name: "commit_list",
  description:
    "List commits of a GitLab project, newest first, from a branch, tag or sha, optionally only " +
    "those touching a path, by an author, or within a date range. Paged. Use commit_read for one " +
    "commit in full.",
  inputSchema: {
    repo: repoArg,
    ref: z.string().optional().describe("Branch, tag or commit sha to list back from. Default: the default branch."),
    path: z.string().optional().describe("Only commits that touched this file or directory."),
    author: z.string().optional().describe("Only commits by this author name or email."),
    since: z.string().optional().describe("Only commits after this ISO 8601 date, e.g. 2026-10-01."),
    until: z.string().optional().describe("Only commits before this ISO 8601 date."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(
      z.object({
        sha: z.string(),
        message: z.string(),
        author: z.string().optional(),
        date: z.string().optional(),
      }),
    ),
    ...pagingOut,
  },
  annotations: READ,
};

/**
 * @param {{ repo?: unknown, ref?: unknown, path?: unknown, author?: unknown, since?: unknown,
 *   until?: unknown, page?: unknown, per_page?: unknown }} args
 */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const ref = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");
  const path = filePath(args.path, "path", { allowEmpty: true }).path || undefined;
  const author = optionalText(args.author, "author");
  const since = optionalDate(args.since, "since");
  const until = optionalDate(args.until, "until");
  const page = pageQuery(args);

  const { body, headers } = await gitlab("GET", `${repo.path}/repository/commits`, {
    query: { ref_name: ref, path, author, since, until, ...page },
    what: `list the commits of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((commit) => ({
    sha: commit.id,
    message: commit.title ?? "",
    ...(commit.author_name ? { author: commit.author_name } : {}),
    ...(commit.authored_date ? { date: commit.authored_date } : {}),
  }));

  return {
    payload: {
      summary:
        items.length === 0
          ? `No commits${ref ? ` on ${ref}` : ""} of ${repo.full} match on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "commit" : "commits"}${ref ? ` on ${ref}` : ""} of ${repo.full} on page ${page.page}.`,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
