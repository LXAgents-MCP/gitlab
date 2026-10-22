/** List what is in a directory of a repository. */
import { z } from "zod";
import { filePath, gitlab, pageQuery, paging, parseRepo, refName } from "../services/api.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

// GitLab's names for what a tree holds, in the words a path is spoken in.
const KINDS = { tree: "dir", blob: "file", commit: "submodule" };

export const TOOL = {
  name: "directory_list",
  description:
    "List the files and folders in one directory of a GitLab project, on the default branch or at a " +
    "branch, tag or commit. Not recursive. Paged. Omit the path for the repository root. Use it to find " +
    "a path before file_read.",
  inputSchema: {
    repo: repoArg,
    path: z.string().optional().describe("Directory inside the repository, e.g. 'src/tools'. Omit for the root."),
    ref: z.string().optional().describe("Branch, tag or commit sha. Default: the default branch."),
    ...pagingArgs,
  },
  outputSchema: {
    path: z.string(),
    items: z.array(z.object({ name: z.string(), type: z.string(), path: z.string() })),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, path?: unknown, ref?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const dir = filePath(args.path, "path", { allowEmpty: true });
  const ref = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");
  const page = pageQuery(args);
  const where = dir.path === "" ? "the root" : `'${dir.path}'`;

  const { body, headers } = await gitlab("GET", `${repo.path}/repository/tree`, {
    query: { path: dir.path, ref, ...page },
    what: `list ${where} of ${repo.full}`,
    explain: (status, detail) =>
      status === 404
        ? `There is no directory ${where} in ${repo.full}${ref ? ` at ${ref}` : ""} (404)${detail}. Check the path and the ref; ` +
          "a file cannot be listed. GitLab also answers 404 for a private project the token cannot see."
        : undefined,
  });

  const items = (Array.isArray(body) ? body : []).map((entry) => ({
    name: entry.name,
    type: KINDS[entry.type] ?? String(entry.type ?? "unknown"),
    path: entry.path ?? entry.name,
  }));

  return {
    payload: {
      summary:
        items.length === 0
          ? `No entries in ${where} of ${repo.full}${ref ? ` at ${ref}` : ""} on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "entry" : "entries"} in ${where} of ${repo.full}${ref ? ` at ${ref}` : ""}, on page ${page.page}.`,
      path: dir.path,
      items,
      ...paging(page, headers),
    },
    ok: true,
  };
}
