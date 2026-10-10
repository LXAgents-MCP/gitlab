/** Change a project's settings. */
import { z } from "zod";
import { SEGMENT, gitlab, parseRepo, refName } from "../api.js";
import { maybeFlag, oneOf, optionalText, rawText } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { describeProject, expectBody, projectFields, projectShape } from "../shapes.js";

const VISIBILITIES = ["private", "internal", "public"];

export const TOOL = {
  name: "repo_update",
  description:
    "Change a GitLab project's settings: rename it or move its URL path, set its description, make it " +
    "private, internal or public, change the default branch, archive or unarchive it, or switch issues " +
    "on or off. Pass only what should change; at least one setting is required.",
  inputSchema: {
    repo: repoArg,
    name: z.string().optional().describe("New display name. It does not change the URL; use `path` for that."),
    path: z
      .string()
      .optional()
      .describe("New URL path: letters, digits, '_', '-' and '.'. Changing it changes the project's address."),
    description: z.string().optional().describe("New description. Pass an empty string to clear it."),
    visibility: z.enum(VISIBILITIES).optional().describe("private, internal or public."),
    default_branch: z.string().optional().describe("Branch to make the default. It must already exist."),
    archived: z.boolean().optional().describe("true archives the project (read-only), false unarchives it."),
    has_issues: z.boolean().optional().describe("Whether the issue tracker is enabled."),
  },
  outputSchema: projectShape,
  annotations: UPDATE,
};

/**
 * @param {{ repo?: unknown, name?: unknown, path?: unknown, description?: unknown, visibility?: unknown,
 *   default_branch?: unknown, archived?: unknown, has_issues?: unknown }} args
 */
export async function handle(args) {
  const repo = parseRepo(args.repo);

  const name = optionalText(args.name, "name");
  const path = optionalText(args.path, "path");
  if (path !== undefined && (!SEGMENT.test(path) || /\.(git|atom)$/i.test(path))) {
    throw new Refused(
      `\`path\` '${path}' is not a valid GitLab project path. Use letters, digits, '_', '-' and '.', ` +
        "not starting with '-' or '.' and not ending in '.git' or '.atom'.",
    );
  }
  const description = rawText(args.description, "description");
  const visibility = oneOf(args.visibility, "visibility", VISIBILITIES);
  const defaultBranch = args.default_branch === undefined ? undefined : refName(args.default_branch, "default_branch");
  const archived = maybeFlag(args.archived, "archived");
  const hasIssues = maybeFlag(args.has_issues, "has_issues");

  const changes = {
    name,
    path,
    description,
    visibility,
    default_branch: defaultBranch,
    archived,
    has_issues: hasIssues,
  };
  const settings = {
    name,
    path,
    description,
    visibility,
    default_branch: defaultBranch,
    ...(hasIssues === undefined ? {} : { issues_access_level: hasIssues ? "enabled" : "disabled" }),
  };
  const json = Object.fromEntries(Object.entries(settings).filter(([, value]) => value !== undefined));
  const changed = Object.entries(changes)
    .filter(([, value]) => value !== undefined)
    .map(([key]) => key);
  if (changed.length === 0) {
    throw new Refused(
      "Nothing to change: pass at least one of name, path, description, visibility, default_branch, " +
        "archived or has_issues.",
    );
  }

  // An archived project is read-only, so it is unarchived before its settings change and archived
  // after. Once the first answer names the project, later calls use its id, which a new path cannot move.
  let project;
  let target = repo.path;
  const first = async (method, suffix, what, body) => {
    const { body: answer } = await gitlab(method, `${target}${suffix}`, { json: body, what });
    project = expectBody(answer, "path_with_namespace", "a project");
    target = `/projects/${project.id}`;
  };

  if (archived === false) await first("POST", "/unarchive", `unarchive the project ${repo.full}`);
  if (Object.keys(json).length > 0) {
    try {
      await first("PUT", "", `update the project ${repo.full}`, json);
    } catch (error) {
      if (error instanceof Refused && archived === false) {
        throw new Refused(`${error.message} The project had already been unarchived.`);
      }
      throw error;
    }
  }
  if (archived === true) {
    try {
      await first("POST", "/archive", `archive the project ${repo.full}`);
    } catch (error) {
      if (error instanceof Refused && Object.keys(json).length > 0) {
        throw new Refused(`${error.message} The other settings had already been changed.`);
      }
      throw error;
    }
  }

  const fields = projectFields(project);
  return {
    payload: { summary: `Updated ${fields.full_name}: ${changed.join(", ")}. ${describeProject(fields)}`, ...fields },
    ok: true,
  };
}
