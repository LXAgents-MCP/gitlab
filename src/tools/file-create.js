/** Create a file as one commit. */
import { z } from "zod";
import { branchOrDefault, filePath, gitlab, parseRepo, refName } from "../api.js";
import { rawText, requiredText } from "../args.js";
import { encodeContent, explainCommit, filesPath } from "../contents.js";
import { WRITE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

export const TOOL = {
  name: "file_create",
  description:
    "Create a new file in a GitLab project as one commit, on a branch (default: the default branch). " +
    "Fails if the file already exists; use file_update to change it. Content up to 1 MB.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the new file, e.g. 'docs/guide.md'."),
    content: z.string().describe("The file's whole content, as text."),
    message: z.string().describe("Commit message."),
    branch: z.string().optional().describe("Branch to commit to. Default: the default branch."),
  },
  outputSchema: {
    path: z.string(),
    branch: z.string(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, path?: unknown, content?: unknown, message?: unknown, branch?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const content = rawText(args.content, "content", { required: true, allowEmpty: true });
  const message = requiredText(args.message, "message");
  const given = args.branch === undefined || args.branch === null ? undefined : refName(args.branch, "branch");
  const encoded = encodeContent(content);

  const branch = await branchOrDefault(repo, given);
  const { body } = await gitlab("POST", filesPath(repo, file), {
    json: { branch, content: encoded, encoding: "base64", commit_message: message },
    what: `create '${file.path}' in ${repo.full} on ${branch}`,
    explain: (status, detail) => (status === 400 ? explainCommit(detail, repo, file, branch) : undefined),
  });

  expectBody(body, "file_path", "a file commit");
  return {
    payload: {
      summary: `Created ${file.path} in ${repo.full} on ${body.branch ?? branch}.`,
      path: body.file_path,
      branch: body.branch ?? branch,
    },
    ok: true,
  };
}
