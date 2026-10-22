/** Create a release. */
import { z } from "zod";
import { branchOrDefault, gitlab, parseRepo, refName } from "../services/api.js";
import { optionalDate, optionalText, rawText } from "../models/args.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { describeRelease, expectBody, releaseFields, releaseShape } from "../models/shapes.js";

export const TOOL = {
  name: "release_create",
  description:
    "Create a release for a tag in a GitLab project, creating the tag from a branch or commit if it " +
    "does not exist. A release date in the future makes it an upcoming release. This is also how a tag " +
    "is created. GitLab has no draft or prerelease flag and does not write notes.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().describe("Tag for the release, e.g. 'v1.2.0'. Created if it does not exist."),
    target: z
      .string()
      .optional()
      .describe("Branch or commit sha to create the tag from. Default: the default branch. Ignored if the tag exists."),
    name: z.string().optional().describe("Release title. Default: the tag."),
    body: z.string().optional().describe("Release notes, in Markdown."),
    released_at: z
      .string()
      .optional()
      .describe("ISO 8601 date of the release. A future date makes it an upcoming release; a past one a historical release."),
  },
  outputSchema: releaseShape,
  annotations: WRITE,
};

/** @param {{ repo?: unknown, tag?: unknown, target?: unknown, name?: unknown, body?: unknown, released_at?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const tag = refName(args.tag, "tag");
  const target = args.target === undefined || args.target === null ? undefined : refName(args.target, "target");
  const name = optionalText(args.name, "name");
  const body = rawText(args.body, "body");
  const releasedAt = optionalDate(args.released_at, "released_at");

  // GitLab wants the starting point named whenever the tag might not exist yet, and ignores it when it does.
  const ref = await branchOrDefault(repo, target);
  const json = Object.fromEntries(
    Object.entries({ tag_name: tag, ref, name, description: body, released_at: releasedAt }).filter(([, value]) => value !== undefined),
  );

  const { body: created } = await gitlab("POST", `${repo.path}/releases`, {
    json,
    what: `create a release for the tag '${tag}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 409 || (status === 400 && /already exists/i.test(detail))
        ? `GitLab could not create a release for '${tag}' in ${repo.full} (${status})${detail}. A release for that tag ` +
          "already exists; read it with release_read, or use another tag."
        : undefined,
  });

  expectBody(created, "tag_name", "a release");
  const fields = releaseFields(created);
  return { payload: { summary: `Created ${describeRelease(fields)} in ${repo.full}.`, ...fields }, ok: true };
}
