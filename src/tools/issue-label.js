/** Change the labels on an issue. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { labelList, oneOf, requiredInt } from "../args.js";
import { Refused } from "../errors.js";
import { UPDATE, repoArg } from "../schemas.js";
import { expectBody, namesIn } from "../shapes.js";

const ACTIONS = ["add", "remove", "set"];

export const TOOL = {
  name: "issue_label",
  description:
    "Add, remove or replace the labels on an issue in a GitLab project. GitLab creates a label that does " +
    "not exist yet, so a misspelled label becomes a new one. The result lists the labels now on the issue " +
    "and any requested label that is not as asked.",
  inputSchema: {
    repo: repoArg,
    number: z.number().int().describe("Issue number in the project."),
    labels: z.array(z.string()).describe("Label names, e.g. ['bug', 'good first issue']. They may not contain a comma. With action set, [] removes every label."),
    action: z
      .enum(ACTIONS)
      .optional()
      .describe("add (default) keeps the current labels and adds these; remove takes these off; set replaces all."),
  },
  outputSchema: {
    number: z.number(),
    labels: z.array(z.string()),
    not_applied: z.array(z.string()).optional(),
  },
  annotations: UPDATE,
};

/** @param {{ repo?: unknown, number?: unknown, labels?: unknown, action?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const number = requiredInt(args.number, "number", { min: 1 });
  const action = /** @type {"add" | "remove" | "set"} */ (oneOf(args.action, "action", ACTIONS, "add"));
  const labels = labelList(args.labels, "labels");
  if (labels === undefined) {
    throw new Refused("`labels` is required and must be a list of label names; with action set, [] removes every label.");
  }
  if (labels.length === 0 && action !== "set") {
    throw new Refused("`labels` must list at least one label. Only action set may be empty, to remove every label.");
  }

  const names = labels.join(",");
  const json = action === "add" ? { add_labels: names } : action === "remove" ? { remove_labels: names } : { labels: names };
  const { body } = await gitlab("PUT", `${repo.path}/issues/${number}`, {
    json,
    what: `${action} labels on #${number} of ${repo.full}`,
  });

  expectBody(body, "web_url", "an issue");
  const now = namesIn(body.labels, "name");
  const has = (name) => now.some((entry) => entry.toLowerCase() === name.toLowerCase());
  const notApplied = action === "remove" ? labels.filter(has) : labels.filter((name) => !has(name));

  return {
    payload: {
      summary:
        `Labels on #${number} in ${repo.full}: ${now.length > 0 ? now.join(", ") : "none"}.` +
        (notApplied.length > 0
          ? ` ${action === "remove" ? "GitLab kept" : "GitLab did not apply"}: ${notApplied.join(", ")}.`
          : ""),
      number,
      labels: now,
      ...(notApplied.length > 0 ? { not_applied: notApplied } : {}),
    },
    ok: true,
  };
}
