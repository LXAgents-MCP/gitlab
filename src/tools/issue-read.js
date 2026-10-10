/** Read one issue, and optionally its discussion. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { optionalFlag, requiredInt } from "../args.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody, issueFields, issueShape } from "../shapes.js";

const COMMENTS_SHOWN = 30;

export const TOOL = {
  name: "issue_read",
  description:
    "Read one issue of a GitLab project: title, state, author, labels, assignees, milestone and body, " +
    "and with `comments: true` its first 30 comments, without the system notes GitLab adds when " +
    "something changes. The number is the issue's number in the project (#42), not its global id; " +
    "merge requests are numbered separately and are read with pr_read.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number in the project, e.g. 42."),
    comments: z.boolean().optional().describe("true also returns the first 30 comments. Default false."),
  },
  outputSchema: {
    ...issueShape,
    comments: z
      .array(z.object({ id: z.number(), author: z.string().optional(), created_at: z.string().optional(), body: z.string() }))
      .optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, number?: unknown, comments?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const withComments = optionalFlag(args.comments, "comments", false);

  const { body } = await gitlab("GET", `${repo.path}/issues/${number}`, {
    what: `read issue #${number} of ${repo.full}`,
  });
  expectBody(body, "web_url", "an issue");
  const fields = issueFields(body);

  let comments;
  if (withComments && fields.comment_count > 0) {
    const answer = await gitlab("GET", `${repo.path}/issues/${number}/notes`, {
      query: { activity_filter: "only_comments", order_by: "created_at", sort: "asc", per_page: COMMENTS_SHOWN },
      what: `read the comments of issue #${number} of ${repo.full}`,
    });
    // `only_comments` is GitLab's own filter; a note marked `system` is dropped as well, in case a
    // version of GitLab ignores the filter.
    comments = (Array.isArray(answer.body) ? answer.body : [])
      .filter((note) => note?.system !== true)
      .map((note) => ({
        id: note.id,
        ...(note.author?.username ? { author: note.author.username } : {}),
        ...(note.created_at ? { created_at: note.created_at } : {}),
        body: note.body ?? "",
      }));
  }

  const more = comments !== undefined && fields.comment_count > comments.length ? `; showing the first ${comments.length} comments` : "";
  return {
    payload: {
      summary: `Issue #${number} in ${repo.full} is ${fields.state}: ${fields.title}${more}.`,
      ...fields,
      ...(comments === undefined ? {} : { comments }),
    },
    ok: true,
  };
}
