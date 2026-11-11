/**
 * The GitLab client: the one place a GitLab URL is built and a call is made.
 *
 * Every tool reaches GitLab through `gitlab()`, and branchOrDefault, which needs a call, and nothing else builds a URL, so nothing else can
 * let an argument steer a call at a different endpoint. The parts of a path come only from
 * `src/models/address.ts`, which validates and percent-encodes them; the request itself is
 * `src/services/client.ts`, and a refusal's wording is `src/services/failure.ts`.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { API_BASE } from "../config/config.js";
import { VERSION } from "../config/version.js";
import { Refused } from "../utils/errors.js";
import { requireToken } from "./auth.js";
import { callApi } from "./client.js";
import { failure } from "./failure.js";
import type { ApiAnswer } from "./client.js";
import type { Repo } from "../models/address.js";

const JSON_ACCEPT = "application/json";

/**
 * The branch a call works on: the one asked for, or else the project's default branch, which a few
 * endpoints will not guess.
 */
export async function branchOrDefault(repo: Repo, branch: string | undefined): Promise<string> {
  if (branch !== undefined) return branch;

  const { body } = await gitlab("GET", repo.path, { what: `find the default branch of ${repo.full}` });
  if (typeof body?.default_branch !== "string" || body.default_branch === "") {
    throw new Refused(`${repo.full} has no default branch yet. Pass \`branch\`, or create the first commit another way.`);
  }
  return body.default_branch;
}

/**
 * Call GitLab and return the answer, or refuse with a message that names the next step.
 *
 * A 2xx answer is returned. So is any status listed in `allow`, for a call where that status is
 * an answer and not a failure - a tag that does not exist, for one. Everything else is refused.
 *
 * The token goes in an `Authorization: Bearer` header, which GitLab accepts for personal, project
 * and group access tokens. The runtime drops that header, and no other, when a redirect changes
 * host; GitLab's own `PRIVATE-TOKEN` header would be forwarded.
 *
 * @param path Starts with `/` and is built only from `parseRepo`, `refPath`, `filePath`
 *   and numbers, so it needs no further encoding. No query string: use `query`.
 * @param options `what` completes "... to <what>" in a refusal.
 */
/** How one call to GitLab is made, and how its failure is worded. */
export interface GitlabOptions {
  query?: Record<string, unknown>;
  json?: unknown;
  accept?: string;
  allow?: number[];
  what?: string;
  explain?: (status: number, detail: string) => string | undefined;
  raw?: boolean;
}

export async function gitlab(method: string, path: string, options: GitlabOptions = {}): Promise<ApiAnswer> {
  const { query, json, accept = JSON_ACCEPT, allow = [], what = "complete the request", explain, raw = false } = options;
  if (!path.startsWith("/") || /[?#]/.test(path)) {
    throw new Error(`gitlab(): path must start with '/' and carry no query or fragment, got '${path}'.`);
  }

  const token = requireToken();
  const url = new URL(`${API_BASE}${path}`);
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") url.searchParams.set(key, String(value));
  }

  const answer = await callApi(url.href, {
    method,
    token,
    json,
    raw,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: accept,
      "User-Agent": `lxagents-mcp-gitlab/${VERSION}`,
    },
  });

  if ((answer.status >= 200 && answer.status < 300) || allow.includes(answer.status)) return answer;
  throw failure({ ...answer, what, explain, token });
}
