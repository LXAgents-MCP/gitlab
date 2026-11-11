/** Delete a release. */
import { z } from "zod";
import { gitlab } from "../services/api.js";
import { parseRepo, refName, refPath } from "../models/address.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import type { ToolDefinition, ToolOutcome } from "./index.js";

export const TOOL: ToolDefinition = {
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

export async function handle(args: { repo?: unknown; tag?: unknown }): Promise<ToolOutcome> {
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
