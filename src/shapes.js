/**
 * GitLab's objects as this server reports them: a schema for a tool's output, and the function
 * that fills it. Kept apart from the tools so every tool that returns a project (or, later, a
 * commit or an issue) says the same things about it, and a field GitLab renames is changed once.
 */
import { z } from "zod";
import { Refused } from "./errors.js";

/**
 * Check that an answer is the kind of object the tool expected, before reading fields off it.
 *
 * GitLab answers 2xx with a body of the shape documented, and a proxy or an outage can answer
 * 2xx with something else. Reading `body.path_with_namespace` off a string would crash; this turns
 * it into a refusal that says what was wrong.
 *
 * @param {any} body
 * @param {string} field A string field the object must have.
 * @param {string} what What the object should have been, for the message.
 * @returns {any} The same body.
 */
export function expectBody(body, field, what) {
  if (!body || typeof body !== "object" || typeof body[field] !== "string") {
    throw new Refused(`GitLab answered, but the body was not ${what} this server recognizes.`);
  }
  return body;
}

/** What every tool that returns a project reports about it. */
export const projectShape = {
  id: z.number(),
  full_name: z.string(),
  description: z.string().optional(),
  html_url: z.string(),
  clone_url: z.string(),
  visibility: z.string(),
  default_branch: z.string().optional(),
  fork: z.boolean(),
  archived: z.boolean(),
  stars: z.number(),
  forks: z.number(),
  open_issues: z.number(),
  license: z.string().optional(),
  topics: z.array(z.string()),
  last_activity_at: z.string().optional(),
  parent: z.string().optional(),
  scheduled_for_deletion: z.string().optional(),
};

/**
 * @param {any} project A project as GitLab returns it.
 * @returns {z.infer<z.ZodObject<typeof projectShape>>}
 */
export function projectFields(project) {
  const topics = Array.isArray(project.topics) ? project.topics : Array.isArray(project.tag_list) ? project.tag_list : [];
  const deletion = project.marked_for_deletion_on ?? project.marked_for_deletion_at;
  return {
    id: project.id,
    full_name: project.path_with_namespace,
    ...(project.description ? { description: project.description } : {}),
    html_url: project.web_url,
    clone_url: project.http_url_to_repo,
    visibility: project.visibility ?? "unknown",
    ...(typeof project.default_branch === "string" ? { default_branch: project.default_branch } : {}),
    fork: Boolean(project.forked_from_project),
    archived: Boolean(project.archived),
    stars: project.star_count ?? 0,
    forks: project.forks_count ?? 0,
    open_issues: project.open_issues_count ?? 0,
    ...(project.license?.name ? { license: project.license.name } : {}),
    topics,
    ...(project.last_activity_at ? { last_activity_at: project.last_activity_at } : {}),
    ...(project.forked_from_project?.path_with_namespace ? { parent: project.forked_from_project.path_with_namespace } : {}),
    ...(typeof deletion === "string" && deletion !== "" ? { scheduled_for_deletion: deletion } : {}),
  };
}

/**
 * One line about a project, for a summary.
 *
 * @param {ReturnType<typeof projectFields>} fields
 * @returns {string}
 */
export function describeProject(fields) {
  const kind = fields.fork ? "fork" : "project";
  const archived = fields.archived ? "archived " : "";
  const branch = fields.default_branch ? `, default branch ${fields.default_branch}` : "";
  const deletion = fields.scheduled_for_deletion ? `, scheduled for deletion on ${fields.scheduled_for_deletion}` : "";
  return `${fields.full_name} is a ${fields.visibility} ${archived}${kind}${branch}${deletion}.`;
}

/** What every tool that returns a release reports about it. */
export const releaseShape = {
  tag: z.string(),
  name: z.string().optional(),
  upcoming: z.boolean(),
  html_url: z.string().optional(),
  released_at: z.string().optional(),
  body: z.string().optional(),
  assets: z.array(z.object({ name: z.string(), url: z.string() })),
};

/**
 * @param {any} release A release as GitLab returns it.
 * @returns {z.infer<z.ZodObject<typeof releaseShape>>} `assets` are the links attached to it and the
 *   source archives GitLab makes, which have no name of their own.
 */
export function releaseFields(release) {
  const links = Array.isArray(release.assets?.links) ? release.assets.links : [];
  const sources = Array.isArray(release.assets?.sources) ? release.assets.sources : [];
  return {
    tag: release.tag_name,
    ...(release.name ? { name: release.name } : {}),
    upcoming: Boolean(release.upcoming_release),
    ...(release._links?.self ? { html_url: release._links.self } : {}),
    ...(release.released_at ? { released_at: release.released_at } : {}),
    ...(release.description ? { body: release.description } : {}),
    assets: [
      ...links.map((link) => ({ name: link.name, url: link.direct_asset_url ?? link.url })),
      ...sources.map((source) => ({ name: `source.${source.format}`, url: source.url })),
    ],
  };
}

/**
 * One line about a release, for a summary.
 *
 * @param {ReturnType<typeof releaseFields>} fields
 * @returns {string}
 */
export function describeRelease(fields) {
  return `${fields.upcoming ? "upcoming release" : "release"} ${fields.name ?? fields.tag} (tag ${fields.tag})`;
}

/**
 * Names out of a list GitLab sends either as plain strings (labels) or as objects (users).
 *
 * @param {unknown} list
 * @param {string} key The field that holds the name in an object: `name` or `username`.
 * @returns {string[]}
 */
export function namesIn(list, key) {
  if (!Array.isArray(list)) return [];
  return list
    .map((entry) => (typeof entry === "string" ? entry : entry?.[key]))
    .filter((name) => typeof name === "string" && name !== "");
}

/** What `issue_read` reports about an issue. */
export const issueShape = {
  number: z.number(),
  title: z.string(),
  state: z.string(),
  author: z.string().optional(),
  labels: z.array(z.string()),
  assignees: z.array(z.string()),
  milestone: z.string().optional(),
  comment_count: z.number(),
  locked: z.boolean(),
  confidential: z.boolean(),
  html_url: z.string(),
  created_at: z.string().optional(),
  updated_at: z.string().optional(),
  closed_at: z.string().optional(),
  body: z.string().optional(),
};

/**
 * @param {any} issue An issue as GitLab returns it. `number` is its `iid`, the number in the
 *   project that people write as #42; `id` is global and is not what any tool takes.
 * @returns {z.infer<z.ZodObject<typeof issueShape>>}
 */
export function issueFields(issue) {
  return {
    number: issue.iid,
    title: issue.title,
    state: issue.state,
    ...(issue.author?.username ? { author: issue.author.username } : {}),
    labels: namesIn(issue.labels, "name"),
    assignees: namesIn(issue.assignees, "username"),
    ...(issue.milestone?.title ? { milestone: issue.milestone.title } : {}),
    comment_count: issue.user_notes_count ?? 0,
    locked: Boolean(issue.discussion_locked),
    confidential: Boolean(issue.confidential),
    html_url: issue.web_url,
    ...(issue.created_at ? { created_at: issue.created_at } : {}),
    ...(issue.updated_at ? { updated_at: issue.updated_at } : {}),
    ...(issue.closed_at ? { closed_at: issue.closed_at } : {}),
    ...(issue.description ? { body: issue.description } : {}),
  };
}
