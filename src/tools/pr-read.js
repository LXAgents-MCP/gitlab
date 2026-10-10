/** Read one merge request. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { optionalFlag, requiredInt } from "../args.js";
import { MAX_COMMENTS_SHOWN, MAX_FILES_SHOWN } from "../config.js";
import { summarizeFile } from "../diffs.js";
import { mergeability } from "../merge-status.js";
import { READ, repoArg } from "../schemas.js";
import { commentShape, commentsFrom, expectBody, namesIn } from "../shapes.js";

export const TOOL = {
  name: "pr_read",
  description:
    "Read one merge request of a GitLab project: state, draft, whether it can be merged and if not why, " +
    "branches, labels, reviewers and description; with `files: true` the first 100 changed files with line " +
    "counts, and with `comments: true` its first 30 comments. The number is the merge request's number in " +
    "the project (!7), which is separate from issue numbers.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Merge request number in the project, e.g. 7 for !7."),
    files: z.boolean().optional().describe("true also lists the changed files, with line counts. Default false."),
    comments: z.boolean().optional().describe("true also returns the first 30 comments, without system notes. Default false."),
  },
  outputSchema: {
    number: z.number(),
    title: z.string(),
    state: z.string(),
    draft: z.boolean(),
    merged: z.boolean(),
    mergeable: z.boolean().optional(),
    merge_status: z.string().optional(),
    author: z.string().optional(),
    head: z.string(),
    base: z.string(),
    head_sha: z.string(),
    changed_files: z.string().optional(),
    labels: z.array(z.string()),
    assignees: z.array(z.string()),
    reviewers: z.array(z.string()),
    comment_count: z.number(),
    html_url: z.string(),
    created_at: z.string().optional(),
    updated_at: z.string().optional(),
    merged_at: z.string().optional(),
    body: z.string().optional(),
    files: z
      .array(z.object({ filename: z.string(), status: z.string(), additions: z.number(), deletions: z.number() }))
      .optional(),
    comments: z.array(commentShape).optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, number?: unknown, files?: unknown, comments?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const withFiles = optionalFlag(args.files, "files", false);
  const withComments = optionalFlag(args.comments, "comments", false);
  const path = `${repo.path}/merge_requests/${number}`;

  const { body } = await gitlab("GET", path, { what: `read merge request !${number} of ${repo.full}` });
  expectBody(body, "web_url", "a merge request");
  const commentCount = body.user_notes_count ?? 0;

  let files;
  if (withFiles) {
    const answer = await gitlab("GET", `${path}/diffs`, {
      query: { per_page: MAX_FILES_SHOWN },
      what: `list the files of merge request !${number} of ${repo.full}`,
    });
    files = (Array.isArray(answer.body) ? answer.body : []).map(summarizeFile);
  }

  let comments;
  if (withComments && commentCount > 0) {
    const answer = await gitlab("GET", `${path}/notes`, {
      query: { activity_filter: "only_comments", order_by: "created_at", sort: "asc", per_page: MAX_COMMENTS_SHOWN },
      what: `read the comments of merge request !${number} of ${repo.full}`,
    });
    comments = commentsFrom(answer.body);
  }

  const merged = body.state === "merged";
  const open = body.state === "opened";
  const draft = Boolean(body.draft ?? body.work_in_progress);
  const verdict = mergeability(body);
  // While GitLab is still working it out, "can it be merged" has no answer yet, which is not "no".
  const known = open && !verdict.pending;
  const changed = body.changes_count === undefined || body.changes_count === null ? undefined : String(body.changes_count);
  const head = body.source_branch ?? "";
  const base = body.target_branch ?? "";

  return {
    payload: {
      summary:
        `Merge request !${number} in ${repo.full} is ${draft && open ? "a draft, opened" : body.state}: ${body.title} (${head} into ${base}).` +
        (open && verdict.pending ? " Whether it can be merged is still being worked out; ask again in a moment." : "") +
        (known && !verdict.ready ? ` It cannot be merged yet: ${verdict.reason}.` : "") +
        (comments !== undefined && commentCount > comments.length ? ` Showing the first ${comments.length} comments.` : ""),
      number,
      title: body.title,
      state: body.state,
      draft,
      merged,
      ...(known ? { mergeable: verdict.ready, merge_status: verdict.status } : {}),
      ...(body.author?.username ? { author: body.author.username } : {}),
      head,
      base,
      head_sha: body.sha ?? "",
      ...(changed === undefined ? {} : { changed_files: changed }),
      labels: namesIn(body.labels, "name"),
      assignees: namesIn(body.assignees, "username"),
      reviewers: namesIn(body.reviewers, "username"),
      comment_count: commentCount,
      html_url: body.web_url,
      ...(body.created_at ? { created_at: body.created_at } : {}),
      ...(body.updated_at ? { updated_at: body.updated_at } : {}),
      ...(body.merged_at ? { merged_at: body.merged_at } : {}),
      ...(body.description ? { body: body.description } : {}),
      ...(files === undefined ? {} : { files }),
      ...(comments === undefined ? {} : { comments }),
    },
    ok: true,
  };
}
