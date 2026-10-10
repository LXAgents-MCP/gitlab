/** Open an issue. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { labelList, optionalInt, rawText, requiredText, textList } from "../args.js";
import { WRITE, repoArg } from "../schemas.js";
import { expectBody, namesIn } from "../shapes.js";
import { findUsers } from "../users.js";

export const TOOL = {
  name: "issue_open",
  description:
    "Open a new issue in a GitLab project, with optional body, labels, assignees and milestone. GitLab " +
    "creates a label that does not exist yet, so a misspelled label becomes a new one. A free plan keeps " +
    "one assignee per issue. The result lists what was actually applied.",
  inputSchema: {
    repo: repoArg,
    title: z.string().describe("Issue title."),
    body: z.string().optional().describe("Issue body, in Markdown."),
    labels: z.array(z.string()).optional().describe("Label names to apply. They may not contain a comma."),
    assignees: z.array(z.string()).optional().describe("GitLab usernames to assign. An unknown username refuses the call."),
    milestone: z.number().int().optional().describe("Milestone id (the id GitLab gives the milestone, not its title or its number in the project)."),
  },
  outputSchema: {
    number: z.number(),
    title: z.string(),
    state: z.string(),
    html_url: z.string(),
    labels: z.array(z.string()),
    assignees: z.array(z.string()),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, title?: unknown, body?: unknown, labels?: unknown, assignees?: unknown, milestone?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const title = requiredText(args.title, "title");
  const body = rawText(args.body, "body");
  const labels = labelList(args.labels, "labels");
  const assignees = textList(args.assignees, "assignees", { max: 10 });
  const milestone = optionalInt(args.milestone, "milestone", { min: 1 });

  const people = assignees === undefined || assignees.length === 0 ? [] : await findUsers(assignees, `assign an issue in ${repo.full}`);

  const { body: created } = await gitlab("POST", `${repo.path}/issues`, {
    json: {
      title,
      ...(body === undefined ? {} : { description: body }),
      ...(labels === undefined || labels.length === 0 ? {} : { labels: labels.join(",") }),
      ...(people.length === 0 ? {} : { assignee_ids: people.map((person) => person.id) }),
      ...(milestone === undefined ? {} : { milestone_id: milestone }),
    },
    what: `open an issue in ${repo.full}`,
  });

  expectBody(created, "web_url", "an issue");
  const applied = { labels: namesIn(created.labels, "name"), assignees: namesIn(created.assignees, "username") };
  const same = (a, b) => a.toLowerCase() === b.toLowerCase();
  const dropped = [
    ...(labels ?? []).filter((name) => !applied.labels.some((now) => same(now, name))).map((name) => `label ${name}`),
    ...people.filter((person) => !applied.assignees.some((now) => same(now, person.username))).map((person) => `assignee ${person.username}`),
  ];
  return {
    payload: {
      summary:
        `Opened issue #${created.iid} in ${repo.full}: ${created.title}.` +
        (dropped.length > 0
          ? ` GitLab did not apply: ${dropped.join(", ")}. A free plan keeps one assignee, and a token without permission to create labels cannot add a new one.`
          : ""),
      number: created.iid,
      title: created.title,
      state: created.state,
      html_url: created.web_url,
      ...applied,
    },
    ok: true,
  };
}
