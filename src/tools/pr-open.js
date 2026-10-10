/** Open a merge request. */
import { z } from "zod";
import { branchOrDefault, gitlab, parseRepo, refName } from "../api.js";
import { maybeFlag, rawText, requiredText } from "../args.js";
import { WRITE, repoArg } from "../schemas.js";
import { expectBody } from "../shapes.js";

// GitLab marks a draft by its title; it also reads these spellings, so none is marked twice.
const DRAFT_TITLE = /^\s*(draft:|\[draft\]|\(draft\))/i;

export const TOOL = {
  name: "pr_open",
  description:
    "Open a merge request in a GitLab project from a branch of the same project into a base branch " +
    "(default: the default branch), optionally as a draft. A draft is a title that starts with 'Draft:', " +
    "which this adds. Fails if one is already open for that branch. A branch of a fork is not supported.",
  inputSchema: {
    repo: repoArg,
    title: z.string().describe("Merge request title."),
    head: z.string().describe("Branch with the changes, e.g. 'feat/login'."),
    base: z.string().optional().describe("Branch to merge into. Default: the project's default branch."),
    body: z.string().optional().describe("Description, in Markdown."),
    draft: z.boolean().optional().describe("true opens it as a draft. Default false."),
  },
  outputSchema: {
    number: z.number(),
    title: z.string(),
    state: z.string(),
    draft: z.boolean(),
    head: z.string(),
    base: z.string(),
    html_url: z.string(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, title?: unknown, head?: unknown, base?: unknown, body?: unknown, draft?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const title = requiredText(args.title, "title");
  const head = refName(args.head, "head");
  const given = args.base === undefined || args.base === null ? undefined : refName(args.base, "base");
  const body = rawText(args.body, "body");
  const draft = maybeFlag(args.draft, "draft");

  const base = await branchOrDefault(repo, given);
  const { body: created } = await gitlab("POST", `${repo.path}/merge_requests`, {
    json: {
      source_branch: head,
      target_branch: base,
      title: draft === true && !DRAFT_TITLE.test(title) ? `Draft: ${title}` : title,
      ...(body === undefined ? {} : { description: body }),
    },
    what: `open a merge request from '${head}' into '${base}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 409
        ? `GitLab would not open a merge request from '${head}' into '${base}' in ${repo.full} (409)${detail}. ` +
          "One is already open for that branch; find it with pr_read."
        : undefined,
  });

  expectBody(created, "web_url", "a merge request");
  const isDraft = Boolean(created.draft ?? created.work_in_progress);
  return {
    payload: {
      summary: `Opened ${isDraft ? "draft " : ""}merge request !${created.iid} in ${repo.full}: ${head} into ${base}.`,
      number: created.iid,
      title: created.title,
      state: created.state,
      draft: isDraft,
      head,
      base,
      html_url: created.web_url,
    },
    ok: true,
  };
}
