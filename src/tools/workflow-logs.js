/** Read the logs of the jobs of a CI/CD pipeline. */
import { z } from "zod";
import { gitlab, parseRepo } from "../api.js";
import { optionalFlag, optionalInt, requiredInt } from "../args.js";
import { MAX_LOG_CHARS, MAX_LOG_JOBS, MAX_PER_PAGE } from "../config.js";
import { Refused } from "../errors.js";
import { READ, repoArg } from "../schemas.js";
import { jobFields, jobShape } from "../shapes.js";
import { cleanLog, tail } from "../text.js";

const isFailed = (job) => job.status === "failed";

export const TOOL = {
  name: "workflow_logs",
  description:
    "Read the logs of the jobs of a GitLab CI/CD pipeline. By default only the failed jobs, up to 5, each cut to its " +
    "last lines, where a failure is explained; job_id reads one job and only_failed false reads every job. At most " +
    "20,000 characters in all. Find pipeline and job ids with workflow_status.",
  inputSchema: {
    repo: repoArg,
    pipeline_id: z.number().int().describe("Pipeline id."),
    job_id: z.number().int().optional().describe("Read just this job of the pipeline. Default: the failed jobs."),
    only_failed: z
      .boolean()
      .optional()
      .describe("false reads every job of the pipeline, not only the failed ones. Default true. Ignored with job_id."),
  },
  outputSchema: {
    pipeline_id: z.number(),
    job_count: z.number(),
    jobs: z.array(
      z.object({ ...jobShape, log: z.string().optional(), truncated: z.boolean(), note: z.string().optional() }),
    ),
    truncated: z.boolean(),
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, pipeline_id?: unknown, job_id?: unknown, only_failed?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const pipelineId = requiredInt(args.pipeline_id, "pipeline_id", { min: 1 });
  const jobId = optionalInt(args.job_id, "job_id", { min: 1 });
  const onlyFailed = optionalFlag(args.only_failed, "only_failed", true);

  const { body: listing } = await gitlab("GET", `${repo.path}/pipelines/${pipelineId}/jobs`, {
    query: { per_page: MAX_PER_PAGE },
    what: `list the jobs of pipeline ${pipelineId} of ${repo.full}`,
    explain: (status, detail) =>
      status === 404
        ? `GitLab could not find pipeline ${pipelineId} in ${repo.full} (404)${detail}. Find pipeline ids with ` +
          "workflow_status or workflow_list. GitLab also answers 404 for a private project the token cannot see."
        : undefined,
  });
  const all = Array.isArray(listing) ? listing : [];

  let selected;
  if (jobId !== undefined) {
    selected = all.filter((job) => job.id === jobId);
    if (selected.length === 0) {
      throw new Refused(
        `Job ${jobId} is not among the ${all.length} jobs listed for pipeline ${pipelineId} in ${repo.full}. ` +
          "Read the pipeline with workflow_status to see its jobs.",
      );
    }
  } else {
    selected = onlyFailed ? all.filter(isFailed) : all;
  }

  if (selected.length === 0) {
    const unfinished = all.some((job) => !["success", "failed", "canceled", "skipped", "manual"].includes(job.status));
    return {
      payload: {
        summary:
          all.length === 0
            ? `Pipeline ${pipelineId} in ${repo.full} has no jobs yet.`
            : `Pipeline ${pipelineId} in ${repo.full} has no failed jobs${unfinished ? " so far, and some jobs have not finished" : ""}. ` +
              "Pass only_failed false to read every job, or job_id for one.",
        pipeline_id: pipelineId,
        job_count: 0,
        jobs: [],
        truncated: false,
      },
      ok: true,
    };
  }

  // The jobs share the budget, one request each and one after another, so a matrix of failures
  // is not a burst against the rate limit.
  const reading = selected.slice(0, MAX_LOG_JOBS);
  const share = Math.floor(MAX_LOG_CHARS / reading.length);
  const jobs = [];
  let cut = selected.length > reading.length;

  for (const job of reading) {
    const fields = jobFields(job);
    const answer = await gitlab("GET", `${repo.path}/jobs/${job.id}/trace`, {
      raw: true,
      allow: [404, 410],
      what: `read the log of job ${job.id} of ${repo.full}`,
    });

    if (answer.status === 404 || answer.status === 410) {
      jobs.push({ ...fields, truncated: false, note: "GitLab has no log for this job: it may not have started, or the log has been erased." });
      continue;
    }
    const text = cleanLog(typeof answer.body === "string" ? answer.body : "").trimEnd();
    if (text === "") {
      jobs.push({ ...fields, truncated: false, note: "The log is empty." });
      continue;
    }
    const [log, shortened] = tail(text, share);
    cut = cut || shortened;
    jobs.push({ ...fields, log, truncated: shortened });
  }

  const summary =
    jobId !== undefined
      ? `Log of job '${jobs[0].name}' (${jobId}) in pipeline ${pipelineId} of ${repo.full}.`
      : `Logs of ${jobs.length} ${onlyFailed ? "failed " : ""}${jobs.length === 1 ? "job" : "jobs"} in pipeline ${pipelineId} of ${repo.full}.` +
        (selected.length > reading.length
          ? ` Showing the first ${reading.length} of ${selected.length}; pass job_id for the others.`
          : "");
  return {
    payload: {
      summary: jobs.some((job) => job.truncated) ? `${summary} A long log is cut to its last ${share} characters.` : summary,
      pipeline_id: pipelineId,
      job_count: selected.length,
      jobs,
      truncated: cut,
    },
    ok: true,
  };
}
