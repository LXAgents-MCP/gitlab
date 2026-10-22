/** Start a CI/CD pipeline. */
import { z } from "zod";
import { branchOrDefault, gitlab, parseRepo, refName } from "../services/api.js";
import { MAX_PIPELINE_VARIABLES } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { WRITE, repoArg } from "../models/schemas.js";
import { expectBody } from "../models/shapes.js";

const VARIABLE_NAME = /^[A-Za-z0-9_]{1,255}$/;

/**
 * The variables of a new pipeline. GitLab takes every value as text, as a job reads it from its
 * environment, so a number or a flag is written out and checked by the job, not here.
 *
 * @param {unknown} value
 * @returns {{ key: string, value: string }[] | undefined}
 */
function pipelineVariables(value) {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new Refused("`variables` must be an object of variable names and their values.");
  }
  const entries = Object.entries(value);
  if (entries.length > MAX_PIPELINE_VARIABLES) {
    throw new Refused(`\`variables\` may hold at most ${MAX_PIPELINE_VARIABLES} variables, got ${entries.length}.`);
  }
  const variables = [];
  for (const [key, entry] of entries) {
    if (!VARIABLE_NAME.test(key)) {
      throw new Refused(`\`variables\` has a name '${key}' that is not letters, digits and underscores, 1 to 255 long.`);
    }
    if (!["string", "number", "boolean"].includes(typeof entry)) {
      throw new Refused(`\`variables.${key}\` must be a string, a number or true/false.`);
    }
    variables.push({ key, value: String(entry) });
  }
  return variables.length === 0 ? undefined : variables;
}

export const TOOL = {
  name: "workflow_run",
  description:
    "Start a CI/CD pipeline of a GitLab project on a branch or tag (default: the default branch), with optional " +
    "variables. It runs whatever .gitlab-ci.yml does on that ref, which may deploy or publish. The ref's " +
    ".gitlab-ci.yml must allow a pipeline started through the API. Returns the pipeline id; follow it with " +
    "workflow_status.",
  inputSchema: {
    repo: repoArg,
    ref: z.string().optional().describe("Branch or tag to run it on. Default: the project's default branch."),
    variables: z
      .record(z.string(), z.union([z.string(), z.number(), z.boolean()]))
      .optional()
      .describe("CI/CD variables for this pipeline by name, up to 25. Values are sent as text and shown in the pipeline, so never a secret."),
  },
  outputSchema: {
    pipeline_id: z.number(),
    status: z.string(),
    ref: z.string(),
    sha: z.string().optional(),
    html_url: z.string(),
  },
  annotations: WRITE,
};

/** @param {{ repo?: unknown, ref?: unknown, variables?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const given = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");
  const variables = pipelineVariables(args.variables);

  const ref = await branchOrDefault(repo, given);
  const { body: created } = await gitlab("POST", `${repo.path}/pipeline`, {
    json: { ref, ...(variables === undefined ? {} : { variables }) },
    what: `start a pipeline on '${ref}' in ${repo.full}`,
    explain: (status, detail) =>
      status === 400
        ? `GitLab would not start a pipeline on '${ref}' in ${repo.full} (400)${detail}. The ref must exist, its ` +
          ".gitlab-ci.yml must be valid and give the pipeline at least one job, and the project's workflow rules must " +
          "allow a pipeline started through the API. Setting variables can also be limited to a higher role."
        : undefined,
  });

  expectBody(created, "web_url", "a pipeline");
  return {
    payload: {
      summary: `Started pipeline ${created.id} on ${ref} in ${repo.full}; it is ${created.status}.`,
      pipeline_id: created.id,
      status: created.status,
      ref,
      ...(created.sha ? { sha: created.sha } : {}),
      html_url: created.web_url,
    },
    ok: true,
  };
}
