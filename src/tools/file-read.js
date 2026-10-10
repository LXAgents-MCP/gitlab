/** Read one text file from a repository. */
import { z } from "zod";
import { filePath, gitlab, parseRepo, refName } from "../api.js";
import { MAX_FILE_CHARS, MAX_READ_BYTES } from "../config.js";
import { filesPath } from "../contents.js";
import { Refused } from "../errors.js";
import { READ, repoArg } from "../schemas.js";
import { cutText, decodeText, looksBinary } from "../text.js";

export const TOOL = {
  name: "file_read",
  description:
    "Read one text file from a GitLab project, on the default branch or at a branch, tag or commit. " +
    "Returns its content, size, blob id and last commit id. Cut at 25,000 characters; binary files and " +
    "files over 1 MB are refused. Use directory_list to find a path first.",
  inputSchema: {
    repo: repoArg,
    path: z.string().describe("Path of the file inside the repository, e.g. 'src/index.js'."),
    ref: z.string().optional().describe("Branch, tag or commit sha. Default: the default branch."),
  },
  outputSchema: {
    path: z.string(),
    blob_id: z.string(),
    last_commit_id: z.string().optional(),
    size: z.number(),
    text: z.string(),
    truncated: z.boolean(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, path?: unknown, ref?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const file = filePath(args.path, "path");
  const ref = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");

  // `HEAD` is the default branch, which saves a request to find its name.
  const { body } = await gitlab("GET", filesPath(repo, file), {
    query: { ref: ref ?? "HEAD" },
    what: `read '${file.path}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 404
        ? `There is no file '${file.path}' in ${repo.full}${ref ? ` at ${ref}` : ""} (404)${detail}. Check the path with ` +
          "directory_list; a directory cannot be read as a file. GitLab also answers 404 for a private project the token cannot see."
        : undefined,
  });

  if (!body || body.encoding !== "base64" || typeof body.content !== "string") {
    throw new Refused("GitLab answered, but the body was not a file this server recognizes.");
  }

  const size = Number.isInteger(body.size) ? body.size : Buffer.byteLength(body.content, "base64");
  if (size > MAX_READ_BYTES) {
    throw new Refused(
      `\`path\` '${file.path}' in ${repo.full} is ${size} bytes, over the ${MAX_READ_BYTES} this tool reads. Fetch it with git instead.`,
    );
  }
  const bytes = Buffer.from(body.content, "base64");
  if (looksBinary(bytes)) {
    throw new Refused(`\`path\` '${file.path}' is a binary file (${bytes.length} bytes); this tool reads text.`);
  }
  const [text, truncated] = cutText(decodeText(bytes), MAX_FILE_CHARS);

  return {
    payload: {
      summary:
        `Read ${file.path} (${size} bytes) from ${repo.full}${ref ? ` at ${ref}` : ""}` +
        (truncated ? `, cut at ${MAX_FILE_CHARS} characters.` : "."),
      path: file.path,
      blob_id: body.blob_id ?? "",
      ...(typeof body.last_commit_id === "string" ? { last_commit_id: body.last_commit_id } : {}),
      size,
      text,
      truncated,
    },
    ok: true,
  };
}
