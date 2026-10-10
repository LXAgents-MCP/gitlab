/**
 * GitLab's file diffs as this server reports them.
 *
 * A diff answer is a list of files, each with its hunks as text and no counts, so the counts are read
 * off the hunks. GitLab starts a file's text at its first `@@`, with no `---` or `+++` header.
 */

/** @param {any} file A file of a diff as GitLab returns it. */
export function fileStatus(file) {
  if (file.new_file) return "added";
  if (file.deleted_file) return "removed";
  if (file.renamed_file) return "renamed";
  return "modified";
}

/**
 * Lines added and removed, counted from the hunks.
 *
 * @param {unknown} diff
 * @returns {{ additions: number, deletions: number }}
 */
export function countLines(diff) {
  let additions = 0;
  let deletions = 0;
  if (typeof diff === "string") {
    for (const line of diff.split("\n")) {
      if (line.startsWith("+")) additions += 1;
      else if (line.startsWith("-")) deletions += 1;
    }
  }
  return { additions, deletions };
}

/**
 * A changed file as a result lists it.
 *
 * @param {any} file
 * @returns {{ filename: string, status: string, additions: number, deletions: number }}
 */
export function summarizeFile(file) {
  return { filename: file.new_path ?? file.old_path ?? "", status: fileStatus(file), ...countLines(file.diff) };
}

/**
 * The files of a diff as one unified diff, with the headers GitLab leaves out.
 *
 * @param {any[]} files
 * @returns {string}
 */
export function unifiedDiff(files) {
  return files
    .map((file) => {
      const from = file.new_file ? "/dev/null" : `a/${file.old_path}`;
      const to = file.deleted_file ? "/dev/null" : `b/${file.new_path}`;
      const body = file.diff
        ? file.diff
        : file.too_large || file.collapsed
          ? "(GitLab left this file's diff out because it is too large)\n"
          : "";
      return `diff --git a/${file.old_path} b/${file.new_path}\n--- ${from}\n+++ ${to}\n${body}`;
    })
    .join("");
}
