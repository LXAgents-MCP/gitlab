import assert from "node:assert/strict";
import test from "node:test";
import { cleanLog } from "../src/utils/text.js";
import { TOOL as list, handle as workflowList } from "../src/controllers/workflow-list.js";
import { TOOL as logs, handle as workflowLogs } from "../src/controllers/workflow-logs.js";
import { TOOL as run, handle as workflowRun } from "../src/controllers/workflow-run.js";
import { TOOL as status, handle as workflowStatus } from "../src/controllers/workflow-status.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const PROJECT = `${API}/projects/g%2Fr`;
const SHA = "0123456789abcdef0123456789abcdef01234567";

/** A pipeline as the list returns it. */
const item = (overrides = {}) => ({
  id: 501,
  iid: 12,
  project_id: 3,
  sha: SHA,
  ref: "main",
  status: "failed",
  source: "push",
  created_at: "2026-10-09T22:15:42.656Z",
  updated_at: "2026-10-10T00:02:34.770Z",
  web_url: "https://gitlab.com/g/r/-/pipelines/501",
  name: null,
  ...overrides,
});

/** A pipeline as reading one returns it. */
const pipeline = (overrides = {}) =>
  item({
    before_sha: "0000000000000000000000000000000000000000",
    tag: false,
    yaml_errors: null,
    user: { id: 5, username: "alice" },
    started_at: "2026-10-09T22:15:45.172Z",
    finished_at: "2026-10-10T00:02:34.743Z",
    duration: 6399,
    ...overrides,
  });

/** A job as the pipeline's job list returns it. */
const job = (overrides = {}) => ({
  id: 9001,
  name: "unit test",
  stage: "test",
  status: "failed",
  allow_failure: false,
  failure_reason: "script_failure",
  duration: 496.477381,
  web_url: "https://gitlab.com/g/r/-/jobs/9001",
  ...overrides,
});

const JOB_FACTS = { id: 9001, name: "unit test", stage: "test", status: "failed", allow_failure: false, failure_reason: "script_failure", duration: 496 };

/* ------------------------------------------------------------------ workflow_list */

test("workflow_list lists pipelines, newest first, and says how many there are", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [item(), item({ id: 500, status: "success", ref: "v1", source: "web" })], { "x-total": "3526", "x-next-page": "2" }));

  const { payload, ok } = await workflowList({ repo: "g/r", per_page: 2 });

  assert.equal(ok, true);
  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/pipelines");
  assert.equal(url.searchParams.get("order_by"), "id");
  assert.equal(url.searchParams.get("sort"), "desc");
  assert.equal(url.searchParams.get("per_page"), "2");
  assert.equal(url.searchParams.has("ref"), false);
  assert.deepEqual(payload, {
    summary: "2 pipelines of g/r, newest first, on page 1 of 3526 in all.",
    items: [
      { id: 501, status: "failed", ref: "main", sha: SHA, source: "push", created_at: "2026-10-09T22:15:42.656Z", html_url: "https://gitlab.com/g/r/-/pipelines/501" },
      { id: 500, status: "success", ref: "v1", sha: SHA, source: "web", created_at: "2026-10-09T22:15:42.656Z", html_url: "https://gitlab.com/g/r/-/pipelines/501" },
    ],
    total_count: 3526,
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("workflow_list narrows by ref and status, and says when there is nothing", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, []));

  const { payload } = await workflowList({ repo: "g/r", ref: "feat/x", status: "running" });

  const url = new URL(calls[0].url);
  assert.equal(url.searchParams.get("ref"), "feat/x");
  assert.equal(url.searchParams.get("status"), "running");
  assert.equal(payload.summary, "g/r has no pipelines on feat/x that are running on page 1.");
  assert.equal("total_count" in payload, false);
  assert.equal(payload.has_more, false);
  expectOutput(list, payload);
});

test("workflow_list refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(workflowList({ repo: "g/r", status: "done" }), isRefusal(/`status` must be one of created, .*, got 'done'/));
  await assert.rejects(workflowList({ repo: "g/r", ref: "a..b" }), isRefusal(/`ref` 'a\.\.b' is not a valid/));
  await assert.rejects(workflowList({ repo: "g/r", per_page: 500 }), isRefusal(/`per_page` must be between/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ workflow_run */

test("workflow_run starts a pipeline on the default branch with variables", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(201, pipeline({ status: "created", finished_at: null })));

  const { payload, ok } = await workflowRun({ repo: "g/r", variables: { DEPLOY: "no", RETRIES: 3, VERBOSE: true } });

  assert.equal(ok, true);
  assert.equal(calls[0].url, PROJECT);
  assert.equal(calls[1].url, `${PROJECT}/pipeline`);
  assert.equal(calls[1].method, "POST");
  assert.deepEqual(calls[1].body, {
    ref: "main",
    variables: [
      { key: "DEPLOY", value: "no" },
      { key: "RETRIES", value: "3" },
      { key: "VERBOSE", value: "true" },
    ],
  });
  assert.deepEqual(payload, {
    summary: "Started pipeline 501 on main in g/r; it is created.",
    pipeline_id: 501,
    status: "created",
    ref: "main",
    sha: SHA,
    html_url: "https://gitlab.com/g/r/-/pipelines/501",
  });
  expectOutput(run, payload);
});

test("workflow_run uses the ref it is given and sends no variables when there are none", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, pipeline({ ref: "v1.0.0", status: "pending" })));

  const { payload } = await workflowRun({ repo: "g/r", ref: "v1.0.0", variables: {} });

  assert.equal(calls.length, 1, "the default branch is not looked up");
  assert.deepEqual(calls[0].body, { ref: "v1.0.0" });
  assert.equal(payload.ref, "v1.0.0");
});

test("workflow_run explains a pipeline GitLab will not create", async (t) => {
  withToken(t);
  mockApi(t, reply(400, { message: { base: ["Reference not found"] } }), reply(201, { unexpected: true }));

  await assert.rejects(
    workflowRun({ repo: "g/r", ref: "nope" }),
    isRefusal(/would not start a pipeline on 'nope' in g\/r \(400\): base Reference not found\. The ref must exist, its \.gitlab-ci\.yml must be valid/),
  );
  await assert.rejects(workflowRun({ repo: "g/r", ref: "main" }), isRefusal(/body was not a pipeline this server recognizes/));
});

test("workflow_run refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, pipeline()));

  await assert.rejects(workflowRun({ repo: "g/r", ref: "a b" }), isRefusal(/`ref` 'a b' is not a valid/));
  await assert.rejects(workflowRun({ repo: "g/r", variables: ["A"] }), isRefusal(/`variables` must be an object/));
  await assert.rejects(workflowRun({ repo: "g/r", variables: { "A-B": "1" } }), isRefusal(/name 'A-B' that is not letters, digits and underscores/));
  await assert.rejects(workflowRun({ repo: "g/r", variables: { A: { nested: 1 } } }), isRefusal(/`variables\.A` must be a string, a number or true\/false/));
  const many = Object.fromEntries(Array.from({ length: 26 }, (_, i) => [`V${i}`, "x"]));
  await assert.rejects(workflowRun({ repo: "g/r", variables: many }), isRefusal(/at most 25 variables, got 26/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ workflow_status */

test("workflow_status reads a pipeline by id, with its jobs and which failed", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, pipeline()),
    reply(200, [job(), job({ id: 9002, name: "lint", status: "success", failure_reason: null, duration: 12.2 }), job({ id: 9003, name: "flaky", allow_failure: true })], { "x-total": "3" }),
  );

  const { payload, ok } = await workflowStatus({ repo: "g/r", pipeline_id: 501 });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${PROJECT}/pipelines/501`);
  const jobsUrl = new URL(calls[1].url);
  assert.equal(jobsUrl.pathname, "/api/v4/projects/g%2Fr/pipelines/501/jobs");
  assert.equal(jobsUrl.searchParams.get("per_page"), "100");
  assert.deepEqual(payload, {
    summary: "Pipeline 501 on main at 0123456 in g/r is failed. Failed: unit test.",
    found: true,
    pipeline_id: 501,
    status: "failed",
    ref: "main",
    sha: SHA,
    source: "push",
    user: "alice",
    created_at: "2026-10-09T22:15:42.656Z",
    started_at: "2026-10-09T22:15:45.172Z",
    finished_at: "2026-10-10T00:02:34.743Z",
    duration: 6399,
    html_url: "https://gitlab.com/g/r/-/pipelines/501",
    job_count: 3,
    jobs: [
      JOB_FACTS,
      { id: 9002, name: "lint", stage: "test", status: "success", allow_failure: false, duration: 12 },
      { ...JOB_FACTS, id: 9003, name: "flaky", allow_failure: true },
    ],
  });
  expectOutput(status, payload);
});

test("workflow_status reads the latest pipeline on a ref, and says when there is none", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [item({ id: 502, status: "running" })]),
    reply(200, pipeline({ id: 502, status: "running", name: "Nightly", finished_at: null, duration: null, yaml_errors: "jobs:x config should be a hash" })),
    reply(200, []),
    reply(200, []),
  );

  const { payload } = await workflowStatus({ repo: "g/r", ref: "main" });

  const listUrl = new URL(calls[0].url);
  assert.equal(listUrl.pathname, "/api/v4/projects/g%2Fr/pipelines");
  assert.equal(listUrl.searchParams.get("ref"), "main");
  assert.equal(listUrl.searchParams.get("per_page"), "1");
  assert.equal(calls[1].url, `${PROJECT}/pipelines/502`);
  assert.match(payload.summary, /^Pipeline 502 \(Nightly\) on main at 0123456 in g\/r is running\.$/);
  assert.equal(payload.yaml_errors, "jobs:x config should be a hash");
  assert.equal("finished_at" in payload, false);
  assert.equal("duration" in payload, false);
  assert.equal(payload.job_count, 0);
  expectOutput(status, payload);

  const none = await workflowStatus({ repo: "g/r", ref: "feat/x" });
  assert.equal(none.ok, true);
  assert.equal(none.payload.found, false);
  assert.match(none.payload.summary, /^No pipeline on feat\/x in g\/r yet\./);
  assert.equal(calls.length, 4);
  expectOutput(status, none.payload);
});

test("workflow_status refuses a missing pipeline in words, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "404 Not found" }));

  await assert.rejects(workflowStatus({ repo: "g/r" }), isRefusal(/Give `pipeline_id`, or `ref`/));
  await assert.rejects(workflowStatus({ repo: "g/r", pipeline_id: 1, ref: "main" }), isRefusal(/either `pipeline_id` or `ref`, not both/));
  await assert.rejects(workflowStatus({ repo: "g/r", pipeline_id: 0 }), isRefusal(/`pipeline_id` must be between 1/));
  await assert.rejects(workflowStatus({ repo: "g/r", ref: "a..b" }), isRefusal(/`ref` 'a\.\.b' is not a valid/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    workflowStatus({ repo: "g/r", pipeline_id: 99 }),
    isRefusal(/could not find pipeline 99 in g\/r \(404\): 404 Not found\. Find pipeline ids with workflow_list/),
  );
});

/* ------------------------------------------------------------------ workflow_logs */

test("workflow_logs reads the log of each failed job, cleaned and cut to its end", async (t) => {
  withToken(t);
  const trace =
    "section_start:1700000000:step_script\r\u001b[0K\u001b[36;1mRunning tests\u001b[0;m\r\n" + "ok line\r\n".repeat(5000) + "\u001b[31;1mERROR: Job failed: exit code 1\u001b[0;m\r\n";
  const { calls } = mockApi(
    t,
    reply(200, [job(), job({ id: 9002, name: "lint", status: "success" }), job({ id: 9004, name: "build", failure_reason: "runner_system_failure" })]),
    reply(200, trace, { "content-type": "text/plain" }),
    reply(200, "building\nERROR: runner went away\n", { "content-type": "text/plain" }),
  );

  const { payload, ok } = await workflowLogs({ repo: "g/r", pipeline_id: 501 });

  assert.equal(ok, true);
  assert.equal(calls[0].url.split("?")[0], `${PROJECT}/pipelines/501/jobs`);
  assert.equal(calls[1].url, `${PROJECT}/jobs/9001/trace`);
  assert.equal(calls[2].url, `${PROJECT}/jobs/9004/trace`);
  assert.equal(payload.job_count, 2);
  assert.equal(payload.jobs.length, 2);
  assert.deepEqual(payload.jobs.map((entry) => entry.name), ["unit test", "build"]);

  const [first, second] = payload.jobs;
  assert.equal(first.truncated, true);
  assert.ok(first.log.startsWith("(truncated) ..."));
  assert.ok(first.log.endsWith("ERROR: Job failed: exit code 1"), "the end, where the failure is, is kept");
  assert.ok(first.log.length <= 10_000 + 20);
  assert.doesNotMatch(first.log, /\u001b|section_start|\r/);
  assert.equal(second.truncated, false);
  assert.equal(second.log, "building\nERROR: runner went away");
  assert.equal(payload.truncated, true);
  assert.match(payload.summary, /^Logs of 2 failed jobs in pipeline 501 of g\/r\. A long log is cut to its last 10000 characters\.$/);
  expectOutput(logs, payload);
});

test("workflow_logs reads one job by id, or every job when asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [job(), job({ id: 9002, name: "lint", status: "success", failure_reason: null })]),
    reply(200, "all good\n"),
    reply(200, [job(), job({ id: 9002, name: "lint", status: "success", failure_reason: null })]),
    reply(200, "first\n"),
    reply(200, "second\n"),
  );

  const one = await workflowLogs({ repo: "g/r", pipeline_id: 501, job_id: 9002 });
  assert.equal(calls[1].url, `${PROJECT}/jobs/9002/trace`);
  assert.equal(one.payload.summary, "Log of job 'lint' (9002) in pipeline 501 of g/r.");
  assert.equal(one.payload.jobs[0].log, "all good");
  expectOutput(logs, one.payload);

  const every = await workflowLogs({ repo: "g/r", pipeline_id: 501, only_failed: false });
  assert.equal(every.payload.jobs.length, 2);
  assert.match(every.payload.summary, /^Logs of 2 jobs in pipeline 501/);
});

test("workflow_logs says when there is nothing to read, and why a job has no log", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, [job({ status: "success", failure_reason: null }), job({ id: 9005, name: "deploy", status: "running", failure_reason: null })]),
    reply(200, []),
    reply(200, [job()]),
    reply(404, { message: "404 Not Found" }),
    reply(200, [job()]),
    reply(200, "\n\n"),
  );

  const none = await workflowLogs({ repo: "g/r", pipeline_id: 501 });
  assert.equal(none.payload.job_count, 0);
  assert.match(none.payload.summary, /has no failed jobs so far, and some jobs have not finished\. Pass only_failed false/);
  expectOutput(logs, none.payload);

  const empty = await workflowLogs({ repo: "g/r", pipeline_id: 501 });
  assert.equal(empty.payload.summary, "Pipeline 501 in g/r has no jobs yet.");

  const erased = await workflowLogs({ repo: "g/r", pipeline_id: 501 });
  assert.match(erased.payload.jobs[0].note, /GitLab has no log for this job/);
  assert.equal("log" in erased.payload.jobs[0], false);
  expectOutput(logs, erased.payload);

  const blank = await workflowLogs({ repo: "g/r", pipeline_id: 501 });
  assert.equal(blank.payload.jobs[0].note, "The log is empty.");
});

test("workflow_logs reads at most five jobs, and refuses a job of another pipeline and bad arguments", async (t) => {
  withToken(t);
  const many = Array.from({ length: 7 }, (_, i) => job({ id: 100 + i, name: `shard ${i}` }));
  const { calls } = mockApi(
    t,
    reply(200, many),
    ...Array.from({ length: 5 }, (_, i) => reply(200, `log ${i}\n`)),
    reply(200, [job()]),
    reply(404, { message: "404 Not found" }),
  );

  await assert.rejects(workflowLogs({ repo: "g/r" }), isRefusal(/`pipeline_id` is required/));
  await assert.rejects(workflowLogs({ repo: "g/r", pipeline_id: 501, job_id: 0 }), isRefusal(/`job_id` must be between 1/));
  await assert.rejects(workflowLogs({ repo: "g/r", pipeline_id: 501, only_failed: "no" }), isRefusal(/`only_failed` must be true or false/));
  assert.equal(calls.length, 0);

  const { payload } = await workflowLogs({ repo: "g/r", pipeline_id: 501 });
  assert.equal(payload.jobs.length, 5);
  assert.equal(payload.job_count, 7);
  assert.equal(payload.truncated, true);
  assert.match(payload.summary, /Showing the first 5 of 7; pass job_id for the others\./);

  await assert.rejects(
    workflowLogs({ repo: "g/r", pipeline_id: 501, job_id: 77 }),
    isRefusal(/Job 77 is not among the 1 jobs listed for pipeline 501 in g\/r/),
  );
  await assert.rejects(
    workflowLogs({ repo: "g/r", pipeline_id: 999 }),
    isRefusal(/could not find pipeline 999 in g\/r \(404\): 404 Not found\. Find pipeline ids with workflow_status or workflow_list/),
  );
});

/* ------------------------------------------------------------------ cleanLog */

test("cleanLog drops colour codes, section markers, the byte order mark and carriage returns", () => {
  const raw =
    "﻿section_start:1700000000:prepare_executor\r\u001b[0K\u001b[36;1mPreparing\u001b[0;m\r\n" +
    "section_start:1700000001:step_script[collapsed=true]\r\u001b[0Kline\r\n" +
    "section_end:1700000002:step_script\r\u001b[0K\n" +
    "\u001b[31;1mERROR: Job failed\u001b[0;m\n";
  assert.equal(cleanLog(raw), "Preparing\nline\n\nERROR: Job failed\n");
  assert.equal(cleanLog("plain\n"), "plain\n");
});
