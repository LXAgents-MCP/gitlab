/** Replace a file's content as one commit. */
import { z } from "zod";
import { filePath, gitlab, parseRepo, refName } from "../api.js";
import { optionalText, rawText, requiredText } from "../args.js";
import { branchOrDefault, encodeContent, explainCommit, filesPath } from "../contents.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

export const TOOL = {
  name: "file_update",
  description:
    "Replace an existing file's whole content in a GitLab project as one commit, on a branch (default: " +
    "the default branch). Pass last_commit_id from file_read to fail if the file changed since. Fails if " +
    "the file does not exist; use file_create for a new one. Content up to 1 MB.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the file to replace."),
    content: z.string().describe("The file's whole new content, as text."),
    message: z.string().describe("Commit message."),
    branch: z.string().optional().describe("Branch to commit to. Default: the default branch."),
    last_commit_id: z
      .string()
      .optional()
      .describe("The file's last_commit_id from file_read. The update fails if the file has changed since."),
  },
  outputSchema: {
    path: z.string(),
    branch: z.string(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, path?: unknown, content?: unknown, message?: unknown, branch?: unknown, last_commit_id?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const content = rawText(args.content, "content", { required: true, allowEmpty: true });
  const message = requiredText(args.message, "message");
  const given = args.branch === undefined || args.branch === null ? undefined : refName(args.branch, "branch");
  const lastCommit = optionalText(args.last_commit_id, "last_commit_id");
  const encoded = encodeContent(content);

  const branch = await branchOrDefault(repo, given);
  const { body } = await gitlab("PUT", filesPath(repo, file), {
    json: {
      branch,
      content: encoded,
      encoding: "base64",
      commit_message: message,
      ...(lastCommit === undefined ? {} : { last_commit_id: lastCommit }),
    },
    what: `update '${file.path}' in ${repo.full} on ${branch}`,
    explain: (status, detail) => (status === 400 ? explainCommit(detail, repo, file, branch) : undefined),
  });

  expectBody(body, "file_path", "a file commit");
  return {
    payload: {
      summary: `Updated ${file.path} in ${repo.full} on ${body.branch ?? branch}.`,
      path: body.file_path,
      branch: body.branch ?? branch,
    },
    ok: true,
  };
}
