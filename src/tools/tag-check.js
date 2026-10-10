/** Say whether a tag exists. */
import { z } from "zod";
import { apiDetail, gitlab, parseRepo, refName, refPath } from "../api.js";
import { Refused } from "../errors.js";
import { READ, repoArg } from "../schemas.js";
import { short } from "../text.js";

export const TOOL = {
  name: "tag_check",
  description:
    "Say whether a tag exists in a GitLab project, and if so what it points at and whether it is " +
    "annotated. A missing tag is an answer, not an error. Use it before creating a release or a tag.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag name, e.g. 'v1.2.0'."),
  },
  outputSchema: {
    exists: z.boolean(),
    tag: z.string(),
    sha: z.string().optional(),
    annotated: z.boolean().optional(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, tag?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");

  // A tag that is not there is the answer to this call, so a 404 is returned and not refused -
  // unless it is the project that is missing.
  const { status, body } = await gitlab("GET", `${repo.path}/repository/tags/${refPath(tag)}`, {
    allow: [404],
    what: `look for the tag '${tag}' in ${repo.full}`,
  });

  if (status === 404) {
    if (/project/i.test(apiDetail(body))) {
      throw new Refused(
        `GitLab could not find the project ${repo.full} (404)${apiDetail(body)}. Check the spelling. GitLab also answers 404 for a private project the token cannot see.`,
      );
    }
    return { payload: { summary: `Tag ${tag} does not exist in ${repo.full}.`, exists: false, tag }, ok: true };
  }

  const sha = body?.commit?.id;
  if (typeof sha !== "string") throw new Refused("GitLab answered, but the body was not a tag this server recognizes.");
  const annotated = typeof body.target === "string" && body.target !== sha;
  return {
    payload: {
      summary: `Tag ${tag} exists in ${repo.full} (${annotated ? "annotated" : "lightweight"}) and points at ${short(sha)}.`,
      exists: true,
      tag,
      sha,
      annotated,
    },
    ok: true,
  };
}
