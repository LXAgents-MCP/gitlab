/** Delete a project. */
import { z } from "zod";
import { gitlab, parseRepo } from "../services/api.js";
import { requiredText } from "../models/args.js";
import { Refused } from "../utils/errors.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";

export const TOOL = {
  name: "repo_delete",
  description:
    "Delete a GitLab project, with its issues, merge requests and wiki. Destructive: confirm with the " +
    "owner first. GitLab may only mark it for deletion and remove it after a retention period, during " +
    "which it can be restored. `confirm` must repeat the project's full path. Needs the Owner role.",
  inputSchema: {
    repo: repoArg,
    confirm: z
      .string()
      .describe("The project's full path again, 'group/project'. Anything else deletes nothing."),
  },
  outputSchema: {
    accepted: z.boolean(),
    full_name: z.string(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, confirm?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const confirm = requiredText(args.confirm, "confirm");

  // A numeric id says nothing a person could type back, so it is resolved to the path first.
  let full = repo.full;
  if (/^\d+$/.test(full)) {
    const { body } = await gitlab("GET", repo.path, { what: `read the project ${repo.full}` });
    full = expectBody(body, "path_with_namespace", "a project").path_with_namespace;
  }

  // The guard is mechanical on purpose: a path typed back cannot be a reflex, a copied flag can.
  if (confirm.toLowerCase() !== full.toLowerCase()) {
    throw new Refused(
      `\`confirm\` must repeat the project's full path, '${full}', to delete it. Nothing was deleted.`,
    );
  }

  await gitlab("DELETE", repo.path, {
    what: `delete the project ${full}`,
    explain: (status, detail) =>
      status === 403
        ? `GitLab refused to delete ${full} (403)${detail}. Deleting needs the Owner role on the project, ` +
          "or the right to delete in its group, and a token with the `api` scope."
        : undefined,
  });

  return {
    payload: {
      summary:
        `GitLab accepted the deletion of ${full}. It may mark the project for deletion and remove it after a ` +
        "retention period, so it can still be restored until then; read it again to see whether it is scheduled.",
      accepted: true,
      full_name: full,
    },
    ok: true,
  };
}
