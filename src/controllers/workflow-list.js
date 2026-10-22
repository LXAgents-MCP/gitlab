/** List a project's recent pipelines. */
import { z } from "zod";
import { gitlab, pageQuery, paging, parseRepo, refName } from "../services/api.js";
import { oneOf } from "../models/args.js";
import { READ, pagingArgs, pagingOut, repoArg } from "../models/schemas.js";

const STATUSES = [
  "created",
  "waiting_for_resource",
  "preparing",
  "pending",
  "running",
  "success",
  "failed",
  "canceled",
  "skipped",
  "manual",
  "scheduled",
];

export const TOOL = {
  name: "workflow_list",
  description:
    "List the recent CI/CD pipelines of a GitLab project, newest first, with their id, status, branch or tag, " +
    "commit and what started them. Paged. A project has one pipeline definition, its .gitlab-ci.yml, so this " +
    "lists runs and not workflows. Use it to find the pipeline to follow with workflow_status.",
  inputSchema: {
    repo: repoArg,
    ref: z.string().optional().describe("Only pipelines on this branch or tag."),
    status: z.enum(STATUSES).optional().describe("Only pipelines in this status, e.g. 'failed' or 'running'."),
    ...pagingArgs,
  },
  outputSchema: {
    items: z.array(
      z.object({
        id: z.number(),
        status: z.string(),
        ref: z.string(),
        sha: z.string(),
        source: z.string().optional(),
        created_at: z.string().optional(),
        html_url: z.string(),
      }),
    ),
    total_count: z.number().optional(),
    ...pagingOut,
  },
  annotations: READ,
};

/** @param {{ repo?: unknown, ref?: unknown, status?: unknown, page?: unknown, per_page?: unknown }} args */
export async function handle(args) {
  const repo = parseRepo(args.repo);
  const ref = args.ref === undefined || args.ref === null ? undefined : refName(args.ref, "ref");
  const status = oneOf(args.status, "status", STATUSES);
  const page = pageQuery(args);

  const { body, headers } = await gitlab("GET", `${repo.path}/pipelines`, {
    query: { ref, status, order_by: "id", sort: "desc", ...page },
    what: `list the pipelines of ${repo.full}`,
  });

  const items = (Array.isArray(body) ? body : []).map((pipeline) => ({
    id: pipeline.id,
    status: pipeline.status,
    ref: pipeline.ref ?? "",
    sha: pipeline.sha ?? "",
    ...(pipeline.source ? { source: pipeline.source } : {}),
    ...(pipeline.created_at ? { created_at: pipeline.created_at } : {}),
    html_url: pipeline.web_url,
  }));
  const total = Number.parseInt(headers["x-total"] ?? "", 10);
  const narrowed = [ref === undefined ? "" : ` on ${ref}`, status === undefined ? "" : ` that are ${status}`].join("");

  return {
    payload: {
      summary:
        items.length === 0
          ? `${repo.full} has no pipelines${narrowed} on page ${page.page}.`
          : `${items.length} ${items.length === 1 ? "pipeline" : "pipelines"}${narrowed} of ${repo.full}, newest first, on page ${page.page}` +
            (Number.isFinite(total) ? ` of ${total} in all.` : "."),
      items,
      ...(Number.isFinite(total) ? { total_count: total } : {}),
      ...paging(page, headers),
    },
    ok: true,
  };
}
