/** Change who an issue is assigned to. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { oneOf, requiredInt, textList } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody, namesIn } from "../shapes.js";
import { bareUsername, findUsers } from "../users.js";

const ACTIONS = ["add", "remove", "set"];

export const TOOL = {
  name: "issue_assign",
  description:
    "Add, remove or replace the assignees of an issue in a GitLab project. An unknown username refuses the " +
    "call before anything changes. A free plan keeps one assignee per issue; the result lists any person " +
    "GitLab did not apply.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number in the project."),
    assignees: z.array(z.string()).describe("GitLab usernames. At most 10. With action set, [] unassigns everyone."),
    action: z
      .enum(ACTIONS)
      .optional()
      .describe("add (default) keeps the current assignees and adds these; remove takes these off; set replaces all."),
  },
  outputSchema: {
    number: z.number(),
    assignees: z.array(z.string()),
    not_applied: z.array(z.string()).optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, assignees?: unknown, action?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const action = /** @type {"add" | "remove" | "set"} */ (oneOf(args.action, "action", ACTIONS, "add"));
  const requested = textList(args.assignees, "assignees", { max: 10 })?.map(bareUsername);
  if (requested === undefined) {
    throw new Refused("`assignees` is required and must be a list of usernames; with action set, [] unassigns everyone.");
  }
  if (requested.length === 0 && action !== "set") {
    throw new Refused("`assignees` must list at least one username. Only action set may be empty, to unassign everyone.");
  }

  const path = `${repo.path}/issues/${number}`;
  const what = `${action} assignees on #${number} of ${repo.full}`;

  // Changing one person means sending the whole list, so add and remove start from who is there now.
  let issue;
  let before = /** @type {{ id: number, username: string }[]} */ ([]);
  if (action !== "set") {
    issue = (await gitlab("GET", path, { what: `read the assignees of #${number} of ${repo.full}` })).body;
    expectBody(issue, "web_url", "an issue");
    before = (Array.isArray(issue.assignees) ? issue.assignees : []).filter((user) => Number.isInteger(user?.id));
  }

  let next;
  if (action === "remove") {
    const drop = new Set(requested.map((name) => name.toLowerCase()));
    next = before.filter((user) => !drop.has(String(user.username).toLowerCase())).map((user) => user.id);
  } else {
    const people = requested.length === 0 ? [] : await findUsers(requested, what);
    next = [...new Set([...(action === "add" ? before.map((user) => user.id) : []), ...people.map((person) => person.id)])];
  }

  const unchanged = action !== "set" && next.length === before.length && next.every((id) => before.some((user) => user.id === id));
  if (!unchanged) {
    // GitLab clears every assignee when it is sent the id 0.
    issue = (await gitlab("PUT", path, { json: { assignee_ids: next.length > 0 ? next : [0] }, what })).body;
  }

  expectBody(issue, "web_url", "an issue");
  const now = namesIn(issue.assignees, "username");
  const has = (list, name) => list.some((entry) => entry.toLowerCase() === name.toLowerCase());
  const wasAssigned = before.map((user) => String(user.username));

  const notApplied =
    action === "remove"
      ? requested.filter((name) => !has(wasAssigned, name) || has(now, name))
      : requested.filter((name) => !has(now, name));

  const note =
    notApplied.length === 0
      ? ""
      : action === "remove"
        ? ` Not removed: ${notApplied.join(", ")}. They were not assigned, or GitLab kept them.`
        : ` GitLab did not assign: ${notApplied.join(", ")}. A free plan keeps one assignee per issue.`;
  return {
    payload: {
      summary: `Assignees of #${number} in ${repo.full}: ${now.length > 0 ? now.join(", ") : "none"}.${note}`,
      number,
      assignees: now,
      ...(notApplied.length > 0 ? { not_applied: notApplied } : {}),
    },
    ok: true,
  };
}
