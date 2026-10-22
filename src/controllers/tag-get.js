/** Read one tag. */
import { z } from "zod";
import { gitlab, parseRepo, refName, refPath } from "../services/api.js";
import { Refused } from "../utils/errors.js";
import { READ, repoArg } from "../models/schemas.js";
import { short } from "../utils/text.js";

export const TOOL = {
  name: "tag_get",
  description:
    "Read one tag of a GitLab project: the commit it points at and, for an annotated tag, its message " +
    "and date, and the release attached to it if there is one. Use it to find the commit behind a " +
    "version tag. tag_check only says whether a tag exists.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag name, e.g. 'v1.2.0'."),
  },
  outputSchema: {
    tag: z.string(),
    sha: z.string(),
    target: z.string(),
    annotated: z.boolean(),
    protected: z.boolean(),
    message: z.string().optional(),
    date: z.string().optional(),
    has_release: z.boolean(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");

  const { body } = await gitlab("GET", `${repo.path}/repository/tags/${refPath(tag)}`, {
    what: `read the tag '${tag}' of ${repo.full}`,
    explain: (status) =>
      status === 404 ? `There is no tag '${tag}' in ${repo.full}. List them with tag_get_all.` : undefined,
  });

  const sha = body?.commit?.id;
  if (typeof sha !== "string") throw new Refused("GitLab answered, but the body was not a tag this server recognizes.");
  // For an annotated tag, `target` is the tag object and not the commit.
  const target = typeof body.target === "string" ? body.target : sha;
  const annotated = target !== sha;
  return {
    payload: {
      summary: `Tag ${tag} is ${annotated ? "annotated" : "a lightweight tag"} and points at ${short(sha)}.`,
      tag,
      sha,
      target,
      annotated,
      protected: Boolean(body.protected),
      ...(annotated && body.message ? { message: body.message } : {}),
      ...(annotated && body.created_at ? { date: body.created_at } : {}),
      has_release: Boolean(body.release),
    },
    ok: true,
  };
}
