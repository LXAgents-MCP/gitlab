/** Delete a file as one commit. */
import { z } from "zod";
import { branchOrDefault, gitlab } from "../services/api.js";
import { filePath, parseRepo, refName } from "../models/address.js";
import { optionalText, requiredText } from "../models/args.js";
import { explainCommit, filesPath } from "../services/contents.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import type { ToolDefinition, ToolOutcome } from "./index.js";

export const TOOL: ToolDefinition = {
  name: "file_delete",
  description:
    "Delete one file from a GitLab project as one commit, on a branch (default: the default branch). " +
    "Destructive: the file stays in history, but confirm with the owner first. Pass last_commit_id from " +
    "file_read to fail if the file changed since.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the file to delete."),
    message: z.string().describe("Commit message."),
    branch: z.string().optional().describe("Branch to commit to. Default: the default branch."),
    last_commit_id: z
      .string()
      .optional()
      .describe("The file's last_commit_id from file_read. The delete fails if the file has changed since."),
  },
  outputSchema: {
    deleted: z.boolean(),
    path: z.string(),
    branch: z.string(),
  },
  annotations: DESTROY,
};

export async function handle(args: { repo?: unknown; path?: unknown; message?: unknown; branch?: unknown; last_commit_id?: unknown }): Promise<ToolOutcome> {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const message = requiredText(args.message, "message");
  const given = args.branch === undefined || args.branch === null ? undefined : refName(args.branch, "branch");
  const lastCommit = optionalText(args.last_commit_id, "last_commit_id");

  const branch = await branchOrDefault(repo, given);
  await gitlab("DELETE", filesPath(repo, file), {
    json: { branch, commit_message: message, ...(lastCommit === undefined ? {} : { last_commit_id: lastCommit }) },
    what: `delete '${file.path}' from ${repo.full} on ${branch}`,
    explain: (status, detail) => (status === 400 ? explainCommit(detail, repo, file, branch) : undefined),
  });

  return {
    payload: { summary: `Deleted ${file.path} from ${repo.full} on ${branch}.`, deleted: true, path: file.path, branch },
    ok: true,
  };
}
