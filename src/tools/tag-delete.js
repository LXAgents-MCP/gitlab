/** Delete a tag. */
import { z } from "zod";
import { gitlab, parseRepo, refName, refPath } from "../api.js";
import { DESTROY, repoArg } from "../schemas.js";

export const TOOL = {
  name: "tag_delete",
  description:
    "Delete a tag from a GitLab project. Destructive: anything that names the tag, such as a build or a " +
    "download link, stops working, so confirm with the owner first. It does not delete a release made " +
    "from the tag. A protected tag cannot be deleted.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Name of the tag to delete."),
  },
  outputSchema: {
    deleted: z.boolean(),
    tag: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");

  await gitlab("DELETE", `${repo.path}/repository/tags/${refPath(tag)}`, {
    what: `delete the tag '${tag}' from ${repo.full}`,
    explain: (status, detail) =>
      status === 403 || status === 404
        ? `GitLab would not delete the tag '${tag}' from ${repo.full} (${status})${detail}. It may not exist ` +
          "(list the tags with tag_get_all), or it is protected, or the token's user may not delete tags."
        : undefined,
  });

  return { payload: { summary: `Deleted tag ${tag} from ${repo.full}.`, deleted: true, tag }, ok: true };
}
