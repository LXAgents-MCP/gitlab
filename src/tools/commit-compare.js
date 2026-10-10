/** Compare two refs. */
import { z } from "zod";
import { gitlab, parseRepo, refName } from "../api.js";
import { Refused } from "../errors.js";
import { MAX_COMMITS_SHOWN, MAX_FILES_SHOWN } from "../config.js";
import { summarizeFile } from "../diffs.js";
import { READ, repoArg } from "../schemas.js";

export const TOOL = {
  name: "commit_compare",
  description:
    "Compare two branches, tags or commits of a GitLab project: how far ahead or behind the head " +
    "is of the base, the most recent 50 commits between them, and the changed files with line " +
    "counts. Use it to see what a branch would bring in before merging.",
  inputSchema: {
    repo: repoArg,
    base: z.string().describe("The ref to compare from, e.g. 'main'."),
    head: z.string().describe("The ref to compare, e.g. 'feat/login'."),
  },
  outputSchema: {
    status: z.string(),
    ahead_by: z.number(),
    behind_by: z.number(),
    total_commits: z.number(),
    commits: z.array(z.object({ sha: z.string(), message: z.string(), author: z.string().optional() })),
    file_count: z.number(),
    files: z.array(
      z.object({ filename: z.string(), status: z.string(), additions: z.number(), deletions: z.number() }),
    ),
    truncated: z.boolean(),
    html_url: z.string().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, base?: unknown, head?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const base = refName(args.base, "base");
  const head = refName(args.head, "head");
  const explain = (status, detail) =>
    status === 404 || status === 400
      ? `GitLab could not find '${base}' or '${head}' in ${repo.full} (${status})${detail}. Check both names, and that the token can see the project.`
      : undefined;

  const { body } = await gitlab("GET", `${repo.path}/repository/compare`, {
    query: { from: base, to: head },
    what: `compare '${head}' with '${base}' in ${repo.full}`,
    explain,
  });
  if (!Array.isArray(body?.commits)) throw new Refused("GitLab answered, but the body was not a comparison this server recognizes.");

  // GitLab compares in one direction, so how far behind is the same question asked the other way round.
  const { body: back } = await gitlab("GET", `${repo.path}/repository/compare`, {
    query: { from: head, to: base },
    what: `compare '${base}' with '${head}' in ${repo.full}`,
    explain,
  });

  const all = body.commits;
  const ahead = all.length;
  const behind = Array.isArray(back?.commits) ? back.commits.length : 0;
  const listed = Array.isArray(body.diffs) ? body.diffs : [];
  // Oldest first, whatever order GitLab sent them in, and the newest are the ones a reader came for.
  const byDate = [...all].sort((a, b) => Date.parse(a.created_at ?? "") - Date.parse(b.created_at ?? "") || 0);
  const commits = byDate.slice(-MAX_COMMITS_SHOWN).map((commit) => ({
    sha: commit.id,
    message: commit.title ?? "",
    ...(commit.author_name ? { author: commit.author_name } : {}),
  }));
  const files = listed.slice(0, MAX_FILES_SHOWN).map(summarizeFile);

  const status = ahead === 0 && behind === 0 ? "identical" : behind === 0 ? "ahead" : ahead === 0 ? "behind" : "diverged";
  const sentence = {
    identical: `${head} and ${base} are identical.`,
    ahead: `${head} is ${ahead} ${ahead === 1 ? "commit" : "commits"} ahead of ${base}.`,
    behind: `${head} is ${behind} ${behind === 1 ? "commit" : "commits"} behind ${base}.`,
    diverged: `${head} has diverged from ${base}: ${ahead} ahead, ${behind} behind.`,
  }[status];

  return {
    payload: {
      summary: sentence,
      status,
      ahead_by: ahead,
      behind_by: behind,
      total_commits: ahead,
      commits,
      file_count: listed.length,
      files,
      truncated: ahead > MAX_COMMITS_SHOWN || listed.length > MAX_FILES_SHOWN || body.compare_timeout === true,
      ...(body.web_url ? { html_url: body.web_url } : {}),
    },
    ok: true,
  };
}
