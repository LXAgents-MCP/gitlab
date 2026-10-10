/**
 * Create a project (a repository) on GitLab.
 *
 * Moved from the `cli` server, where it was `create_gitlab_repo`. The arguments, the defaults
 * and the wording of its refusals are unchanged.
 */
import { z } from "zod";
import { SEGMENT, gitlab } from "../api.js";
import { optionalFlag, optionalText, requiredText } from "../args.js";
import { Refused } from "../errors.js";
import { WRITE } from "../schemas.js";

const VISIBILITIES = ["private", "internal", "public"];
const MAX_GROUP_DEPTH = 20;

export const TOOL = {
  name: "repo_create",
  description:
    "Create a new repository (a project) on GitLab.com and return its URLs. Use this when the " +
    "owner asks to start a new GitLab repository, in their own namespace or in a group. " +
    "Private unless told otherwise. It only creates: it does not clone, push to, change or " +
    "delete anything. The token is read from the LXAGENTS_MCP_GITLAB_API_KEY environment " +
    "variable of the server, never from an argument.",
  inputSchema: {
    name: z
      .string()
      .describe(
        "Repository name, which is also its URL path: letters, digits, '_', '-' and '.', " +
          "not starting with '-' or '.'. Example: 'my-new-repo'.",
      ),
    group: z
      .string()
      .optional()
      .describe(
        "Full path of the group or subgroup to create it in. Omit to create it in the " +
          "personal namespace that owns the token. Example: 'lxagents-mcp' or 'lxagents-mcp/tools'.",
      ),
    description: z.string().optional().describe("One-line description shown on the repository."),
    visibility: z
      .enum(["private", "internal", "public"])
      .optional()
      .describe(
        "Default 'private'. 'internal' is visible to any signed-in GitLab.com user. Pass " +
          "'public' only when the owner asked for a public repository.",
      ),
    initialize_with_readme: z
      .boolean()
      .optional()
      .describe(
        "Default true: create an initial commit with a README, so the repository can be " +
          "cloned straight away. Pass false for a truly empty repository.",
      ),
  },
  outputSchema: {
    name: z.string(),
    full_name: z.string(),
    html_url: z.string(),
    clone_url: z.string(),
    visibility: z.string(),
    default_branch: z.string().optional(),
  },
  annotations: WRITE,
};

/**
 * The wording of the statuses a create can meet, which names the next step for each.
 *
 * @param {number} status
 * @param {string} detail GitLab's own message, as a suffix.
 * @param {{ name: string, group: string | undefined, where: string, step: "group" | "project" }} request
 * @returns {string | undefined} `undefined` leaves a 401 to the general wording.
 */
function explain(status, detail, { name, group, where, step }) {
  if (status === 401) return undefined;
  if (status === 404 && step === "group") {
    return (
      `GitLab could not find the group '${group}' (404), or the token cannot see it. Check ` +
      "the full path, or omit `group` to create the repository in the token owner's namespace."
    );
  }
  if (status === 403) {
    return (
      `GitLab refused the request (403)${detail}. The token needs the \`api\` scope, and its ` +
      `owner must be allowed to create projects ${where}.`
    );
  }
  if (status === 400 || status === 409 || status === 422) {
    return (
      `GitLab could not create '${name}' (${status})${detail}. The most common cause is that ` +
      `a project with that name or path already exists ${where}; pick another name.`
    );
  }
  return `GitLab answered ${status}${detail}.`;
}

/**
 * @param {{ name?: unknown, group?: unknown, description?: unknown, visibility?: unknown, initialize_with_readme?: unknown }} args
 * @returns {Promise<{ payload: object, ok: boolean }>}
 */
export async function handle(args) {
  const name = requiredText(args.name, "name");
  if (!SEGMENT.test(name) || /\.(git|atom)$/i.test(name)) {
    throw new Refused(
      `\`name\` '${name}' is not a valid GitLab repository name. Use letters, digits, '_', ` +
        "'-' and '.', not starting with '-' or '.' and not ending in '.git' or '.atom'.",
    );
  }

  const group = optionalText(args.group, "group");
  if (group !== undefined) {
    const segments = group.split("/");
    if (segments.length > MAX_GROUP_DEPTH || !segments.every((s) => SEGMENT.test(s))) {
      throw new Refused(
        `\`group\` '${group}' is not a valid GitLab group path. Give the full path, such as ` +
          "'my-group' or 'my-group/sub-group'; omit it to create the repository in the token " +
          "owner's namespace.",
      );
    }
  }

  const description = optionalText(args.description, "description");
  const visibility = optionalText(args.visibility, "visibility") ?? "private";
  if (!VISIBILITIES.includes(visibility)) {
    throw new Refused(`\`visibility\` must be one of ${VISIBILITIES.join(", ")}, got '${visibility}'.`);
  }
  const readme = optionalFlag(args.initialize_with_readme, "initialize_with_readme", true);

  const where = group ? `in '${group}'` : "in the token owner's namespace";

  // A project is created in a group by the group's id, not its path, so look it up first.
  let namespaceId;
  if (group !== undefined) {
    const { body: found } = await gitlab("GET", `/groups/${encodeURIComponent(group)}`, {
      what: `find the group '${group}'`,
      explain: (code, detail) => explain(code, detail, { name, group, where, step: "group" }),
    });
    if (!Number.isInteger(found?.id)) {
      throw new Refused(`GitLab answered 200 for the group '${group}', but without an id this server recognizes.`);
    }
    namespaceId = found.id;
  }

  const { body } = await gitlab("POST", "/projects", {
    json: {
      name,
      path: name,
      visibility,
      initialize_with_readme: readme,
      ...(namespaceId === undefined ? {} : { namespace_id: namespaceId }),
      ...(description === undefined ? {} : { description }),
    },
    what: `create the project '${name}' ${where}`,
    explain: (code, detail) => explain(code, detail, { name, group, where, step: "project" }),
  });

  if (!body || typeof body !== "object" || typeof body.web_url !== "string") {
    throw new Refused("GitLab answered 201, but the body was not a project this server recognizes.");
  }

  const created = body.visibility ?? visibility;
  return {
    payload: {
      summary: `Created ${created} repository ${body.path_with_namespace} on GitLab.`,
      name: body.path ?? body.name,
      full_name: body.path_with_namespace,
      html_url: body.web_url,
      clone_url: body.http_url_to_repo,
      visibility: created,
      ...(typeof body.default_branch === "string" ? { default_branch: body.default_branch } : {}),
    },
    ok: true,
  };
}
