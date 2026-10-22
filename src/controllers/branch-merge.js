/** Merge one branch into another, through a merge request that is accepted at once. */
import { z } from "zod";
import { gitlab, parseRepo, refName } from "../services/api.js";
import { optionalText } from "../models/args.js";
import { MERGE_POLL_MS, MERGE_POLL_TRIES } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { mergeability } from "../services/merge-status.js";
import { DESTROY, repoArg } from "../models/schemas.js";
import { short } from "../utils/text.js";
import { sleep } from "../utils/wait.js";

export const TOOL = {
  name: "branch_merge",
  description:
    "Merge one branch into another in a GitLab project. GitLab has no direct merge, so this opens a merge " +
    "request from head into base and accepts it at once; the merged request stays as the record, and if " +
    "it cannot be merged the request is closed again. Destructive: it moves the base branch, so confirm " +
    "with the owner first. Use pr_merge when a merge request exists.",
  inputSchema: {
    repo: repoArg,
    base: z.string().describe("Branch that receives the merge, e.g. 'main'."),
    head: z.string().describe("Branch to merge into the base."),
    message: z.string().optional().describe("Message for the merge commit. Default: GitLab's."),
  },
  outputSchema: {
    merged: z.boolean(),
    up_to_date: z.boolean(),
    sha: z.string().optional(),
    merge_request: z.number().optional(),
    html_url: z.string().optional(),
  },
  annotations: DESTROY,
};

/**
 * Close a merge request this tool opened and could not merge, and say so in words for a refusal.
 *
 * @returns {Promise<string>}
 */
async function closeAgain(repo, iid) {
  try {
    await gitlab("PUT", `${repo.path}/merge_requests/${iid}`, {
      json: { state_event: "close" },
      what: `close the merge request !${iid} of ${repo.full}`,
    });
    return `The merge request it opened, !${iid}, was closed again.`;
  } catch {
    return `The merge request it opened, !${iid}, is still open; close it with pr_close.`;
  }
}

/** @param {{ repo?: unknown, base?: unknown, head?: unknown, message?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const base = refName(args.base, "base");
  const head = refName(args.head, "head");
  const message = optionalText(args.message, "message");

  // Nothing to merge is an answer, and it must not leave a merge request behind.
  const { body: compared } = await gitlab("GET", `${repo.path}/repository/compare`, {
    query: { from: base, to: head },
    what: `compare '${head}' with '${base}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 404 || status === 400
        ? `GitLab could not find '${base}' or '${head}' in ${repo.full} (${status})${detail}. Check both names, ` +
          "and that the token can see the project."
        : undefined,
  });
  if (Array.isArray(compared?.commits) && compared.commits.length === 0) {
    return {
      payload: { summary: `Nothing to merge: ${base} already contains ${head}.`, merged: false, up_to_date: true },
      ok: true,
    };
  }

  const { body: opened } = await gitlab("POST", `${repo.path}/merge_requests`, {
    json: {
      source_branch: head,
      target_branch: base,
      title: `Merge ${head} into ${base}`,
      remove_source_branch: false,
    },
    what: `open a merge request from '${head}' into '${base}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 409
        ? `GitLab could not open a merge request from '${head}' into '${base}' in ${repo.full} (409)${detail}. ` +
          "One is already open; merge that one with pr_merge."
        : undefined,
  });
  if (!Number.isInteger(opened?.iid)) {
    throw new Refused("GitLab answered, but the body was not a merge request this server recognizes.");
  }
  const iid = opened.iid;

  // GitLab works out whether it can merge in the background; ask again until it has, or give up.
  let current = opened;
  for (let tries = 0; mergeability(current).pending && tries < MERGE_POLL_TRIES; tries += 1) {
    await sleep(MERGE_POLL_MS);
    ({ body: current } = await gitlab("GET", `${repo.path}/merge_requests/${iid}`, {
      what: `read the merge request !${iid} of ${repo.full}`,
    }));
  }

  const verdict = mergeability(current);
  if (!verdict.ready) {
    const closed = await closeAgain(repo, iid);
    throw new Refused(
      verdict.pending
        ? `GitLab is still working out whether '${head}' can be merged into '${base}' in ${repo.full}. ${closed} Try again in a moment.`
        : `GitLab will not merge '${head}' into '${base}' in ${repo.full}: ${verdict.reason}. ${closed}`,
    );
  }

  let merged;
  try {
    ({ body: merged } = await gitlab("PUT", `${repo.path}/merge_requests/${iid}/merge`, {
      json: {
        sha: current.sha ?? opened.sha,
        should_remove_source_branch: false,
        ...(message === undefined ? {} : { merge_commit_message: message }),
      },
      what: `merge '${head}' into '${base}' in ${repo.full}`,
      explain: (status, detail) => {
        if (status === 401) {
          return `GitLab would not let this token merge into '${base}' in ${repo.full} (401)${detail}. Merging into a protected branch needs the Maintainer role, or a role the branch's protection allows.`;
        }
        if (status === 405 || status === 422) {
          return `GitLab could not merge '${head}' into '${base}' in ${repo.full} (${status})${detail}. It cannot be merged as it stands.`;
        }
        if (status === 409) {
          return `GitLab did not merge '${head}' into '${base}' in ${repo.full} (409)${detail}. '${head}' moved while the merge was being made; try again.`;
        }
        return undefined;
      },
    }));
  } catch (error) {
    if (error instanceof Refused) throw new Refused(`${error.message} ${await closeAgain(repo, iid)}`);
    throw error;
  }

  const sha = merged?.merge_commit_sha ?? merged?.squash_commit_sha ?? undefined;
  const state = merged?.state;
  const done = state === "merged";
  return {
    payload: {
      summary: done
        ? `Merged ${head} into ${base} in ${repo.full}${sha ? `, at ${short(sha)}` : ""}, through merge request !${iid}.`
        : state === "locked"
          ? `GitLab accepted the merge of ${head} into ${base} in ${repo.full} (merge request !${iid}) and is completing it; read it with pr_read.`
          : `GitLab answered the merge of ${head} into ${base} in ${repo.full} without saying it merged (merge request !${iid} is ${state ?? "in an unknown state"}); read it with pr_read.`,
      merged: done,
      up_to_date: false,
      ...(sha ? { sha } : {}),
      merge_request: iid,
      ...(typeof opened.web_url === "string" ? { html_url: opened.web_url } : {}),
    },
    ok: done || state === "locked",
  };
}
