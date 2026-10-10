/** Delete a release. */
import { z } from "zod";
import { gitlab, parseRepo, refName, refPath } from "../api.js";
import { DESTROY, repoArg } from "../schemas.js";

export const TOOL = {
  name: "release_delete",
  description:
    "Delete the release of a tag in a GitLab project, with its notes and asset links. Destructive, so " +
    "confirm with the owner first. The tag is kept; use tag_delete to remove it.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag of the release to delete."),
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

  await gitlab("DELETE", `${repo.path}/releases/${refPath(tag)}`, {
    what: `delete the release for '${tag}' of ${repo.full}`,
    explain: (status) =>
      status === 404 ? `${repo.full} has no release for the tag '${tag}'. List what exists with release_list.` : undefined,
  });

  return {
    payload: { summary: `Deleted the release for ${tag} of ${repo.full}. Its tag was kept.`, deleted: true, tag },
    ok: true,
  };
}
