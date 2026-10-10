/**
 * The repository files API, which the file tools share.
 *
 * GitLab changes a file by `POST`, `PUT` or `DELETE` against its path, one commit on a branch that
 * must be named, so a tool that is not handed a branch has to find the default one first. That
 * lookup, the path, and the encoding of what is written live here once.
 */
import { gitlab } from "./api.js";
import { MAX_WRITE_BYTES } from "./config.js";
import { Refused } from "./errors.js";

/**
 * The API path of a file: its path written as one encoded segment.
 *
 * @param {{ path: string }} repo
 * @param {{ encoded: string }} file An already-encoded path, from `filePath`.
 * @returns {string}
 */
export function filesPath(repo, file) {
  return `${repo.path}/repository/files/${file.encoded}`;
}

/**
 * Content as the API wants it written, refusing what is too large to be a source file. It is sent as
 * base64, so that the text arrives exactly as it was given, line endings included.
 *
 * @param {string} content
 * @returns {string} Base64.
 */
export function encodeContent(content) {
  const bytes = Buffer.from(content, "utf8");
  if (bytes.length > MAX_WRITE_BYTES) {
    throw new Refused(
      `\`content\` is ${bytes.length} bytes, over the ${MAX_WRITE_BYTES}-byte limit for one write. ` +
        "This tool writes source files; commit large files with git.",
    );
  }
  return bytes.toString("base64");
}

/**
 * The branch a write goes to: the one asked for, or else the project's default.
 *
 * @param {{ full: string, path: string }} repo
 * @param {string | undefined} branch
 * @returns {Promise<string>}
 */
export async function branchOrDefault(repo, branch) {
  if (branch !== undefined) return branch;

  const { body } = await gitlab("GET", repo.path, { what: `find the default branch of ${repo.full}` });
  if (typeof body?.default_branch !== "string" || body.default_branch === "") {
    throw new Refused(`${repo.full} has no default branch yet. Pass \`branch\`, or create the first commit another way.`);
  }
  return body.default_branch;
}

/**
 * The wording of the 400 GitLab answers a file commit with, which names the cause only in its message.
 *
 * @param {string} detail GitLab's own message, as a suffix.
 * @param {{ full: string }} repo
 * @param {{ path: string }} file
 * @param {string} branch
 * @returns {string | undefined} `undefined` leaves it to the general wording.
 */
export function explainCommit(detail, repo, file, branch) {
  if (/already exists/i.test(detail)) {
    return `A file '${file.path}' already exists on ${branch} in ${repo.full}${detail}. Use file_update to change it.`;
  }
  if (/doesn't exist|does not exist|not found/i.test(detail)) {
    return `There is no file '${file.path}' on ${branch} in ${repo.full}${detail}. Use file_create to make it, or directory_list to find it.`;
  }
  if (/changed since/i.test(detail)) {
    return `'${file.path}' on ${branch} in ${repo.full} changed since it was read${detail}. Read it again with file_read and use its new last_commit_id.`;
  }
  return undefined;
}
