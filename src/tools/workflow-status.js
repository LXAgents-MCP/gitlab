/** Read the state of a CI/CD pipeline and its jobs. */
import { z } from "zod";
import { gitlab, parseRepo, refName } from "../api.js";
import { optionalInt, optionalText } from "../args.js";
import { MAX_PER_PAGE } from "../config.js";
import { Refused } from "../errors.js";
import { READ, repoArg } from "../schemas.js";
import { expectBody, jobFields, jobShape } from "../shapes.js";
import { short } from "../text.js";

export const TOOL = {
  name: "workflow_status",
  description:
    "Read a GitLab CI/CD pipeline: its status, branch or tag, commit, timing, and each job's result with the " +
    "reason a failed one failed. Give a pipeline_id, or a ref to read the latest pipeline on that branch or tag. " +
    "After workflow_run, ask again until the status is success, failed, canceled or skipped.",
  inputSchema: {
    repo: repoArg,
    pipeline_id: z.number().int().optional().describe("Pipeline id, from workflow_run, workflow_list or an earlier workflow_status."),
    ref: z.string().optional().describe("Branch or tag; reads its most recent pipeline."),
  },
  outputSchema: {
    found: z.boolean(),
    pipeline_id: z.number().optional(),
    name: z.string().optional(),
    status: z.string().optional(),
    ref: z.string().optional(),
    sha: z.string().optional(),
    source: z.string().optional(),
    user: z.string().optional(),
    created_at: z.string().optional(),
    started_at: z.string().optional(),
    finished_at: z.string().optional(),
    duration: z.number().optional(),
    yaml_errors: z.string().optional(),
    html_url: z.string().optional(),
    job_count: z.number().optional(),
    jobs: z.array(z.object(jobShape)),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, pipeline_id?: unknown, ref?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const pipelineId = optionalInt(args.pipeline_id, "pipeline_id", { min: 1 });
  const ref = optionalText(args.ref, "ref") === undefined ? undefined : refName(args.ref, "ref");

  if (pipelineId !== undefined && ref !== undefined) throw new Refused("Give either `pipeline_id` or `ref`, not both.");
  if (pipelineId === undefined && ref === undefined) {
    throw new Refused("Give `pipeline_id`, or `ref` to read the latest pipeline on a branch or tag.");
  }

  let id = pipelineId;
  if (id === undefined) {
    // Listing answers an empty list for a ref with no pipeline, where asking for "the latest" answers 403 or 404 that
    // cannot be told from a project that cannot be seen.
    const { body } = await gitlab("GET", `${repo.path}/pipelines`, {
      query: { ref, order_by: "id", sort: "desc", per_page: 1 },
      what: `find the latest pipeline on '${ref}' in ${repo.full}`,
    });
    id = Array.isArray(body) ? body[0]?.id : undefined;
    if (!Number.isInteger(id)) {
      return {
        payload: {
          summary:
            `No pipeline on ${ref} in ${repo.full} yet. A pipeline appears a few seconds after it is triggered; ` +
            "ask again shortly.",
          found: false,
          jobs: [],
        },
        ok: true,
      };
    }
  }

  const { body: pipeline } = await gitlab("GET", `${repo.path}/pipelines/${id}`, {
    what: `read pipeline ${id} of ${repo.full}`,
    explain: (status, detail) =>
      status === 404
        ? `GitLab could not find pipeline ${id} in ${repo.full} (404)${detail}. Find pipeline ids with workflow_list. ` +
          "GitLab also answers 404 for a private project the token cannot see."
        : undefined,
  });
  expectBody(pipeline, "web_url", "a pipeline");

  const { body: listing, headers } = await gitlab("GET", `${repo.path}/pipelines/${id}/jobs`, {
    query: { per_page: MAX_PER_PAGE },
    what: `list the jobs of pipeline ${id} of ${repo.full}`,
  });
  const jobs = (Array.isArray(listing) ? listing : []).map(jobFields);
  const total = Number.parseInt(headers["x-total"] ?? "", 10);
  const failed = jobs.filter((job) => job.status === "failed" && !job.allow_failure);

  const name = typeof pipeline.name === "string" && pipeline.name !== "" ? pipeline.name : "";
  const where = pipeline.ref ? ` on ${pipeline.ref}` : "";
  return {
    payload: {
      summary:
        `Pipeline ${pipeline.id}${name ? ` (${name})` : ""}${where} at ${short(pipeline.sha)} in ${repo.full} is ${pipeline.status}.` +
        (failed.length > 0 ? ` Failed: ${failed.map((job) => job.name).join(", ")}.` : ""),
      found: true,
      pipeline_id: pipeline.id,
      ...(name ? { name } : {}),
      status: pipeline.status,
      ...(pipeline.ref ? { ref: pipeline.ref } : {}),
      ...(pipeline.sha ? { sha: pipeline.sha } : {}),
      ...(pipeline.source ? { source: pipeline.source } : {}),
      ...(pipeline.user?.username ? { user: pipeline.user.username } : {}),
      ...(pipeline.created_at ? { created_at: pipeline.created_at } : {}),
      ...(pipeline.started_at ? { started_at: pipeline.started_at } : {}),
      ...(pipeline.finished_at ? { finished_at: pipeline.finished_at } : {}),
      ...(typeof pipeline.duration === "number" ? { duration: pipeline.duration } : {}),
      ...(typeof pipeline.yaml_errors === "string" && pipeline.yaml_errors !== "" ? { yaml_errors: pipeline.yaml_errors } : {}),
      html_url: pipeline.web_url,
      job_count: Number.isFinite(total) ? total : jobs.length,
      jobs,
    },
    ok: true,
  };
}
