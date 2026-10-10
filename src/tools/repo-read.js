/** Read one project. */
import { gitlab, parseRepo } from "../api.js";
import { READ, repoArg } from "../schemas.js";
import { describeProject, expectBody, projectFields, projectShape } from "../shapes.js";

export const TOOL = {
  name: "repo_read",
  description:
    "Read one GitLab project: description, visibility, default branch, star, fork and open issue " +
    "counts, license, topics, last activity and parent. Use it to learn about a repository before " +
    "acting on it.",
  inputSchema: { repo: repoArg },
  outputSchema: projectShape,
  annotations: READ,
};

/** @param {{ repo?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const { body } = await gitlab("GET", repo.path, {
    query: { license: true },
    what: `read the project ${repo.full}`,
  });

  const fields = projectFields(expectBody(body, "path_with_namespace", "a project"));
  return { payload: { summary: describeProject(fields), ...fields }, ok: true };
}
