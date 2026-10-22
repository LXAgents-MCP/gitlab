/** Read one commit. */
import { z } from "zod";
import { gitlab, parseRepo, refName, refPath } from "../services/api.js";
import { optionalFlag } from "../models/args.js";
import { MAX_DIFF_CHARS, MAX_FILES_SHOWN } from "../config/config.js";
import { summarizeFile, unifiedDiff } from "../utils/diffs.js";
import { READ, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";
import { firstLine, head, short } from "../utils/text.js";

export const TOOL = {
  name: "commit_read",
  description:
    "Read one commit of a GitLab project: its message, author, date, parents, the files it changed " +
    "with their line counts, and optionally its diff (cut at 20,000 characters). Use it to see what a " +
    "commit did. Defaults to the tip of the default branch.",
  inputSchema: {
    repo: repoArg,
    ref: z.string().optional().describe("Commit sha, branch or tag. Default: the default branch's tip."),
    diff: z.boolean().optional().describe("true also returns the commit's diff. Default false."),
  },
  outputSchema: {
    sha: z.string(),
    message: z.string(),
    author: z.string().optional(),
    date: z.string().optional(),
    parents: z.array(z.string()),
    additions: z.number(),
    deletions: z.number(),
    file_count: z.number(),
    files: z.array(
      z.object({ filename: z.string(), status: z.string(), additions: z.number(), deletions: z.number() }),
    ),
    diff: z.string().optional(),
    truncated: z.boolean(),
    html_url: z.string().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, ref?: unknown, diff?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const ref = args.ref === undefined || args.ref === null ? "HEAD" : refName(args.ref, "ref");
  const wantDiff = optionalFlag(args.diff, "diff", false);
  const path = `${repo.path}/repository/commits/${refPath(ref)}`;

  const { body } = await gitlab("GET", path, { what: `read the commit '${ref}' of ${repo.full}` });
  expectBody(body, "id", "a commit");

  // The commit carries its totals but not its files, which are the diff's.
  const { body: changed, headers } = await gitlab("GET", `${path}/diff`, {
    query: { per_page: MAX_FILES_SHOWN },
    what: `list the files of the commit '${ref}' of ${repo.full}`,
  });
  const listed = Array.isArray(changed) ? changed : [];
  const files = listed.map(summarizeFile);
  let truncated = (headers["x-next-page"] ?? "") !== "";

  let diff;
  if (wantDiff) {
    const [shown, cut] = head(unifiedDiff(listed), MAX_DIFF_CHARS);
    diff = shown;
    truncated = truncated || cut;
  }

  const subject = body.title ?? firstLine(body.message);
  const author = body.author_name;
  return {
    payload: {
      summary: `Commit ${short(body.id)}${author ? ` by ${author}` : ""}: ${subject || "(no message)"}`,
      sha: body.id,
      message: body.message ?? "",
      ...(author ? { author } : {}),
      ...(body.authored_date ? { date: body.authored_date } : {}),
      parents: Array.isArray(body.parent_ids) ? body.parent_ids : [],
      additions: body.stats?.additions ?? files.reduce((sum, file) => sum + file.additions, 0),
      deletions: body.stats?.deletions ?? files.reduce((sum, file) => sum + file.deletions, 0),
      file_count: files.length,
      files,
      ...(diff === undefined ? {} : { diff }),
      truncated,
      ...(body.web_url ? { html_url: body.web_url } : {}),
    },
    ok: true,
  };
}
