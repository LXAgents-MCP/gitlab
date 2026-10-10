/** Merge a merge request. */
import { z } from "zod";
import { apiDetail, gitlab, parseRepo } from "../api.js";
import { maybeFlag, optionalText, rawText, requiredInt } from "../args.js";
import { Refused } from "../errors.js";
import { mergeability } from "../merge-status.js";
import { DESTROY, repoArg } from "../schemas.js";
import { short } from "../text.js";

// The statuses GitLab answers with when the request cannot be merged as it stands.
const CANNOT_MERGE = [405, 406, 422];

export const TOOL = {
  name: "pr_merge",
  description:
    "Merge a merge request in a GitLab project. How it lands (a merge commit, a fast-forward or a rebase) is " +
    "the project's setting and cannot be chosen here; `squash` joins its commits into one first. The source " +
    "branch is removed only if the merge request was set to. Destructive: it moves the target branch, so confirm " +
    "with the owner first. Pass `sha` to merge only if the branch is still at the commit that was reviewed.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Merge request number in the project."),
    squash: z.boolean().optional().describe("true squashes the commits into one. Default: the merge request's own setting."),
    message: z
      .string()
      .optional()
      .describe("Message for the squash commit when squashing, otherwise for the merge commit. Default: GitLab's."),
    sha: z.string().optional().describe("The head commit the merge must match; a moved branch is refused (409)."),
  },
  outputSchema: {
    merged: z.boolean(),
    state: z.string().optional(),
    sha: z.string().optional(),
  },
  annotations: DESTROY,
};

/** @param {{ repo?: unknown, number?: unknown, squash?: unknown, message?: unknown, sha?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const squash = maybeFlag(args.squash, "squash");
  const message = rawText(args.message, "message");
  const sha = optionalText(args.sha, "sha");
  const path = `${repo.path}/merge_requests/${number}`;
  const what = `merge merge request !${number} of ${repo.full}`;

  const answer = await gitlab("PUT", `${path}/merge`, {
    json: {
      ...(squash === undefined ? {} : { squash }),
      ...(message === undefined ? {} : squash === true ? { squash_commit_message: message } : { merge_commit_message: message }),
      ...(sha === undefined ? {} : { sha }),
    },
    allow: CANNOT_MERGE,
    what,
    explain: (status, detail) => {
      if (status === 401) {
        return (
          `GitLab would not let this token merge !${number} in ${repo.full} (401)${detail}. Either the token is not valid, ` +
          "or its user may not merge into the target branch, which for a protected branch takes the Maintainer role " +
          "or a role the branch's protection allows."
        );
      }
      if (status === 409) {
        return (
          `GitLab did not merge !${number} in ${repo.full} (409)${detail}. The branch moved since the commit that was ` +
          "meant to be merged; read the merge request again with pr_read."
        );
      }
      return undefined;
    },
  });

  if (CANNOT_MERGE.includes(answer.status)) {
    // GitLab says only that it cannot, so ask it why: it knows whether the cause is a draft, a pipeline, an approval or a conflict.
    let why = "";
    try {
      const { body: current } = await gitlab("GET", path, { what: `read merge request !${number} of ${repo.full}` });
      const verdict = mergeability(current);
      why =
        current?.state !== "opened"
          ? ` It is ${current?.state ?? "not open"}.`
          : verdict.ready
            ? " GitLab reports it as mergeable, so the project's merge settings refused it."
            : ` ${verdict.pending ? "GitLab is still working out whether it can be merged; try again in a moment" : `It cannot be merged yet: ${verdict.reason}`}.`;
    } catch {
      // The reason is a courtesy; the refusal stands without it.
    }
    throw new Refused(`GitLab would not ${what} (${answer.status})${apiDetail(answer.body)}.${why}`);
  }

  const body = answer.body;
  const state = typeof body?.state === "string" ? body.state : undefined;
  const merged = state === "merged";
  const commit = body?.merge_commit_sha ?? body?.squash_commit_sha ?? undefined;
  return {
    payload: {
      summary: merged
        ? `Merged merge request !${number} in ${repo.full}${commit ? `, at ${short(commit)}` : ""}.`
        : state === "locked"
          ? `GitLab accepted the merge of !${number} in ${repo.full} and is completing it; read it with pr_read.`
          : `GitLab answered the merge of !${number} in ${repo.full} without saying it merged (it is ${state ?? "in an unknown state"}); read it with pr_read.`,
      merged,
      ...(state ? { state } : {}),
      ...(commit ? { sha: commit } : {}),
    },
    ok: merged || state === "locked",
  };
}
