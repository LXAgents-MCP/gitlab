/** Read one release. */
import { z } from "zod";
import { gitlab } from "../services/api.js";
import { parseRepo, refName, refPath } from "../models/address.js";
import { READ, repoArg } from "../models/schemas.js";
import { describeRelease, expectBody, releaseFields, releaseShape } from "../models/shapes.js";
import type { ToolDefinition, ToolOutcome } from "./index.js";

export const TOOL: ToolDefinition = {
  name: "release_read",
  description:
    "Read one release of a GitLab project: title, notes, release date, and its asset links and source " +
    "archives. Give a tag, or omit it for the latest release.",
  inputSchema: {
    repo: repoArg,
    tag: z.string().optional().describe("Tag of the release. Omit for the latest release."),
  },
  outputSchema: releaseShape,
  annotations: READ,
};

export async function handle(args: { repo?: unknown; tag?: unknown }): Promise<ToolOutcome> {
  const repo = parseRepo(args.repo);
  const tag = args.tag === undefined || args.tag === null ? undefined : refName(args.tag, "tag");

  // The latest release is a permanent address that redirects to the release itself.
  const { body: release } =
    tag === undefined
      ? await gitlab("GET", `${repo.path}/releases/permalink/latest`, {
          what: `read the latest release of ${repo.full}`,
          explain: (status) =>
            status === 404 ? `${repo.full} has no release yet. List what exists with release_list.` : undefined,
        })
      : await gitlab("GET", `${repo.path}/releases/${refPath(tag)}`, {
          what: `read the release for '${tag}' of ${repo.full}`,
          explain: (status) =>
            status === 404 ? `${repo.full} has no release for the tag '${tag}'. List what exists with release_list.` : undefined,
        });

  expectBody(release, "tag_name", "a release");
  const fields = releaseFields(release);
  return { payload: { summary: `${repo.full} has ${describeRelease(fields)}.`, ...fields }, ok: true };
}
