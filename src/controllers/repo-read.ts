/** Read one project. */
import { gitlab } from "../services/api.js";
import { parseRepo } from "../models/address.js";
import { READ, repoArg } from "../models/schemas.js";
import { describeProject, expectBody, projectFields, projectShape } from "../models/shapes.js";
import type { ToolDefinition, ToolOutcome } from "./index.js";

export const TOOL: ToolDefinition = {
  name: "repo_read",
  description:
    "Read one GitLab project: description, visibility, default branch, star, fork and open issue " +
    "counts, license, topics, last activity and parent. Use it to learn about a repository before " +
    "acting on it.",
  inputSchema: { repo: repoArg },
  outputSchema: projectShape,
  annotations: READ,
};

export async function handle(args: { repo?: unknown }): Promise<ToolOutcome> {
  const repo = parseRepo(args.repo);
  const { body } = await gitlab("GET", repo.path, {
    query: { license: true },
    what: `read the project ${repo.full}`,
  });

  const fields = projectFields(expectBody(body, "path_with_namespace", "a project"));
  return { payload: { summary: describeProject(fields), ...fields }, ok: true };
}
