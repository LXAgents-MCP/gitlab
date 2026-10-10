/** Fork a project. */
import { z } from "zod";
import { SEGMENT, gitlab, isNamespacePath, parseRepo } from "../api.js";
import { optionalText } from "../args.js";
import { Refused } from "../errors.js";
import { WRITE, repoArg } from "../schemas.js";
import { expectBody, projectFields, projectShape } from "../shapes.js";

export const TOOL = {
  name: "repo_fork",
  description:
    "Fork a GitLab project into the token owner's namespace or into a group. GitLab copies it in the " +
    "background, so the fork may take a moment to be readable. Use it to work on a repository the " +
    "owner cannot push to.",
  inputSchema: {
    repo: repoArg,
    group: z
      .string()
      .optional()
      .describe("Full path of the group or namespace to fork into. Omit to fork into the token owner's namespace."),
    name: z.string().optional().describe("Name for the fork. Omit to keep the source's name."),
    path: z.string().optional().describe("URL path for the fork. Omit to keep the source's path."),
  },
  outputSchema: { ...projectShape, import_status: z.string().optional() },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, group?: unknown, name?: unknown, path?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const group = optionalText(args.group, "group");
  if (group !== undefined && !isNamespacePath(group)) {
    throw new Refused(
      `\`group\` '${group}' is not a valid GitLab namespace path. Give the full path, such as 'my-group' or ` +
        "'my-group/sub-group'; omit it to fork into the token owner's namespace.",
    );
  }
  const name = optionalText(args.name, "name");
  const path = optionalText(args.path, "path");
  if (path !== undefined && (!SEGMENT.test(path) || /\.(git|atom)$/i.test(path))) {
    throw new Refused(
      `\`path\` '${path}' is not a valid GitLab project path. Use letters, digits, '_', '-' and '.', ` +
        "not starting with '-' or '.' and not ending in '.git' or '.atom'.",
    );
  }

  const json = Object.fromEntries(
    Object.entries({ namespace_path: group, name, path }).filter(([, value]) => value !== undefined),
  );
  const { body } = await gitlab("POST", `${repo.path}/fork`, {
    json,
    what: `fork ${repo.full}`,
  });

  const project = expectBody(body, "path_with_namespace", "a project");
  const fields = projectFields(project);
  return {
    payload: {
      summary:
        `Forking ${repo.full} into ${fields.full_name} has started. GitLab finishes the copy in the ` +
        "background, so give it a moment before reading it.",
      ...fields,
      ...(typeof project.import_status === "string" ? { import_status: project.import_status } : {}),
    },
    ok: true,
  };
}
