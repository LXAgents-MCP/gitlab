/** Delete a tag. */
import { z } from "zod";
import { gitlab } from "../services/api.js";
import { parseRepo, refName, refPath } from "../models/address.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import type { ToolDefinition, ToolOutcome } from "./index.js";

export const TOOL: ToolDefinition = {
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

export async function handle(args: { repo?: unknown; tag?: unknown }): Promise<ToolOutcome> {
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
