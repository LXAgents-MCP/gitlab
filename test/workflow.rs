//! workflow_list, workflow_run, workflow_status and workflow_logs, and the log cleaner.
use lxagents_gitlab::utils::text::clean_log;
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn project() -> String {
    format!("{API}/projects/g%2Fr")
}

/// A pipeline as the list returns it.
fn item(overrides: Value) -> Value {
    with(
        json!({
            "id": 501, "iid": 12, "project_id": 3, "sha": SHA, "ref": "main", "status": "failed", "source": "push",
            "created_at": "2026-10-09T22:15:42.656Z", "updated_at": "2026-10-10T00:02:34.770Z",
            "web_url": "https://gitlab.com/g/r/-/pipelines/501", "name": null,
        }),
        overrides,
    )
}

/// A pipeline as reading one returns it.
fn pipeline(overrides: Value) -> Value {
    with(
        item(json!({
            "before_sha": "0000000000000000000000000000000000000000", "tag": false, "yaml_errors": null,
            "user": { "id": 5, "username": "alice" }, "started_at": "2026-10-09T22:15:45.172Z",
            "finished_at": "2026-10-10T00:02:34.743Z", "duration": 6399,
        })),
        overrides,
    )
}

/// A job as the pipeline's job list returns it.
fn job(overrides: Value) -> Value {
    with(
        json!({
            "id": 9001, "name": "unit test", "stage": "test", "status": "failed", "allow_failure": false,
            "failure_reason": "script_failure", "duration": 496.477381, "web_url": "https://gitlab.com/g/r/-/jobs/9001",
        }),
        overrides,
    )
}

fn job_facts(overrides: Value) -> Value {
    with(
        json!({ "id": 9001, "name": "unit test", "stage": "test", "status": "failed", "allow_failure": false, "failure_reason": "script_failure", "duration": 496 }),
        overrides,
    )
}

fn text(body: &str) -> Reply {
    reply_with(200, json!(body), &[("content-type", "text/plain")])
}

fn summary(payload: &Value) -> &str {
    payload["summary"].as_str().unwrap()
}

/* ------------------------------------------------------------------ workflow_list */

#[tokio::test]
async fn workflow_list_lists_pipelines_newest_first_and_says_how_many_there_are() {
    let t = Harness::with_token();
    t.mock(vec![reply_with(
        200,
        json!([
            item(json!({})),
            item(json!({ "id": 500, "status": "success", "ref": "v1", "source": "web" }))
        ]),
        &[("x-total", "3526"), ("x-next-page", "2")],
    )]);
    let outcome = t
        .outcome("workflow_list", json!({ "repo": "g/r", "per_page": 2 }))
        .await;
    assert!(outcome.ok);
    let call = &t.calls()[0];
    assert_eq!(call.path(), "/api/v4/projects/g%2Fr/pipelines");
    assert_eq!(call.param("order_by").as_deref(), Some("id"));
    assert_eq!(call.param("sort").as_deref(), Some("desc"));
    assert_eq!(call.param("per_page").as_deref(), Some("2"));
    assert!(call.param("ref").is_none());
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "2 pipelines of g/r, newest first, on page 1 of 3526 in all.",
            "items": [
                { "id": 501, "status": "failed", "ref": "main", "sha": SHA, "source": "push", "created_at": "2026-10-09T22:15:42.656Z", "html_url": "https://gitlab.com/g/r/-/pipelines/501" },
                { "id": 500, "status": "success", "ref": "v1", "sha": SHA, "source": "web", "created_at": "2026-10-09T22:15:42.656Z", "html_url": "https://gitlab.com/g/r/-/pipelines/501" },
            ],
            "total_count": 3526, "page": 1, "per_page": 2, "has_more": true,
        })
    );
}

#[tokio::test]
async fn workflow_list_narrows_by_ref_and_status_and_says_when_there_is_nothing() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!([]))]);
    let payload = t
        .ok(
            "workflow_list",
            json!({ "repo": "g/r", "ref": "feat/x", "status": "running" }),
        )
        .await;
    assert_eq!(t.calls()[0].param("ref").as_deref(), Some("feat/x"));
    assert_eq!(t.calls()[0].param("status").as_deref(), Some("running"));
    assert_eq!(
        summary(&payload),
        "g/r has no pipelines on feat/x that are running on page 1."
    );
    assert!(payload.get("total_count").is_none());
    assert_eq!(payload["has_more"], json!(false));
}

#[tokio::test]
async fn workflow_list_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!([]))]);
    assert_match(
        &t.refused("workflow_list", json!({ "repo": "g/r", "status": "done" }))
            .await,
        "`status` must be one of created, .*, got 'done'",
    );
    assert_match(
        &t.refused("workflow_list", json!({ "repo": "g/r", "ref": "a..b" }))
            .await,
        r"`ref` 'a\.\.b' is not a valid",
    );
    assert_match(
        &t.refused("workflow_list", json!({ "repo": "g/r", "per_page": 500 }))
            .await,
        "`per_page` must be between",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ workflow_run */

#[tokio::test]
async fn workflow_run_starts_a_pipeline_on_the_default_branch_with_variables() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(
            201,
            pipeline(json!({ "status": "created", "finished_at": null })),
        ),
    ]);
    let outcome = t.outcome("workflow_run", json!({ "repo": "g/r", "variables": { "DEPLOY": "no", "RETRIES": 3, "VERBOSE": true } })).await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, project());
    assert_eq!(calls[1].url, format!("{}/pipeline", project()));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(
        calls[1].body(),
        &json!({
            "ref": "main",
            "variables": [{ "key": "DEPLOY", "value": "no" }, { "key": "RETRIES", "value": "3" }, { "key": "VERBOSE", "value": "true" }],
        })
    );
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Started pipeline 501 on main in g/r; it is created.", "pipeline_id": 501, "status": "created",
            "ref": "main", "sha": SHA, "html_url": "https://gitlab.com/g/r/-/pipelines/501",
        })
    );
}

#[tokio::test]
async fn workflow_run_uses_the_ref_it_is_given_and_sends_no_variables_when_there_are_none() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        201,
        pipeline(json!({ "ref": "v1.0.0", "status": "pending" })),
    )]);
    let payload = t
        .ok(
            "workflow_run",
            json!({ "repo": "g/r", "ref": "v1.0.0", "variables": {} }),
        )
        .await;
    assert_eq!(t.call_count(), 1, "the default branch is not looked up");
    assert_eq!(t.calls()[0].body(), &json!({ "ref": "v1.0.0" }));
    assert_eq!(payload["ref"], json!("v1.0.0"));
}

#[tokio::test]
async fn workflow_run_explains_a_pipeline_gitlab_will_not_create() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            400,
            json!({ "message": { "base": ["Reference not found"] } }),
        ),
        reply(201, json!({ "unexpected": true })),
    ]);
    assert_match(
        &t.refused("workflow_run", json!({ "repo": "g/r", "ref": "nope" }))
            .await,
        r"would not start a pipeline on 'nope' in g/r \(400\): base Reference not found\. The ref must exist, its \.gitlab-ci\.yml must be valid",
    );
    assert_match(
        &t.refused("workflow_run", json!({ "repo": "g/r", "ref": "main" }))
            .await,
        "body was not a pipeline this server recognizes",
    );
}

#[tokio::test]
async fn workflow_run_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, pipeline(json!({})))]);
    assert_match(
        &t.refused("workflow_run", json!({ "repo": "g/r", "ref": "a b" }))
            .await,
        "`ref` 'a b' is not a valid",
    );
    assert_match(
        &t.refused("workflow_run", json!({ "repo": "g/r", "variables": ["A"] }))
            .await,
        "`variables` must be an object",
    );
    assert_match(
        &t.refused(
            "workflow_run",
            json!({ "repo": "g/r", "variables": { "A-B": "1" } }),
        )
        .await,
        "name 'A-B' that is not letters, digits and underscores",
    );
    assert_match(
        &t.refused(
            "workflow_run",
            json!({ "repo": "g/r", "variables": { "A": { "nested": 1 } } }),
        )
        .await,
        r"`variables\.A` must be a string, a number or true/false",
    );
    let many: serde_json::Map<String, Value> =
        (0..26).map(|i| (format!("V{i}"), json!("x"))).collect();
    assert_match(
        &t.refused("workflow_run", json!({ "repo": "g/r", "variables": many }))
            .await,
        "at most 25 variables, got 26",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ workflow_status */

#[tokio::test]
async fn workflow_status_reads_a_pipeline_by_id_with_its_jobs_and_which_failed() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, pipeline(json!({}))),
        reply_with(
            200,
            json!([
                job(json!({})),
                job(json!({ "id": 9002, "name": "lint", "status": "success", "failure_reason": null, "duration": 12.2 })),
                job(json!({ "id": 9003, "name": "flaky", "allow_failure": true })),
            ]),
            &[("x-total", "3")],
        ),
    ]);
    let outcome = t
        .outcome(
            "workflow_status",
            json!({ "repo": "g/r", "pipeline_id": 501 }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, format!("{}/pipelines/501", project()));
    assert_eq!(calls[1].path(), "/api/v4/projects/g%2Fr/pipelines/501/jobs");
    assert_eq!(calls[1].param("per_page").as_deref(), Some("100"));
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Pipeline 501 on main at 0123456 in g/r is failed. Failed: unit test.", "found": true, "pipeline_id": 501,
            "status": "failed", "ref": "main", "sha": SHA, "source": "push", "user": "alice", "created_at": "2026-10-09T22:15:42.656Z",
            "started_at": "2026-10-09T22:15:45.172Z", "finished_at": "2026-10-10T00:02:34.743Z", "duration": 6399,
            "html_url": "https://gitlab.com/g/r/-/pipelines/501", "job_count": 3,
            "jobs": [
                job_facts(json!({})),
                { "id": 9002, "name": "lint", "stage": "test", "status": "success", "allow_failure": false, "duration": 12 },
                job_facts(json!({ "id": 9003, "name": "flaky", "allow_failure": true })),
            ],
        })
    );
}

#[tokio::test]
async fn workflow_status_reads_the_latest_pipeline_on_a_ref_and_says_when_there_is_none() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!([item(json!({ "id": 502, "status": "running" }))])),
        reply(
            200,
            pipeline(json!({ "id": 502, "status": "running", "name": "Nightly", "finished_at": null, "duration": null, "yaml_errors": "jobs:x config should be a hash" })),
        ),
        reply(200, json!([])),
        reply(200, json!([])),
    ]);
    let payload = t
        .ok("workflow_status", json!({ "repo": "g/r", "ref": "main" }))
        .await;
    let calls = t.calls();
    assert_eq!(calls[0].path(), "/api/v4/projects/g%2Fr/pipelines");
    assert_eq!(calls[0].param("ref").as_deref(), Some("main"));
    assert_eq!(calls[0].param("per_page").as_deref(), Some("1"));
    assert_eq!(calls[1].url, format!("{}/pipelines/502", project()));
    assert_match(
        summary(&payload),
        r"^Pipeline 502 \(Nightly\) on main at 0123456 in g/r is running\.$",
    );
    assert_eq!(
        payload["yaml_errors"],
        json!("jobs:x config should be a hash")
    );
    assert!(payload.get("finished_at").is_none());
    assert!(payload.get("duration").is_none());
    assert_eq!(payload["job_count"], json!(0));
    let none = t
        .outcome("workflow_status", json!({ "repo": "g/r", "ref": "feat/x" }))
        .await;
    assert!(none.ok);
    let none = crate::helpers::payload(&none);
    assert_eq!(none["found"], json!(false));
    assert_match(summary(&none), r"^No pipeline on feat/x in g/r yet\.");
    assert_eq!(t.call_count(), 4);
}

#[tokio::test]
async fn workflow_status_refuses_a_missing_pipeline_in_words_and_bad_arguments_before_any_request()
{
    let t = Harness::with_token();
    t.mock(vec![reply(404, json!({ "message": "404 Not found" }))]);
    assert_match(
        &t.refused("workflow_status", json!({ "repo": "g/r" })).await,
        "Give `pipeline_id`, or `ref`",
    );
    assert_match(
        &t.refused(
            "workflow_status",
            json!({ "repo": "g/r", "pipeline_id": 1, "ref": "main" }),
        )
        .await,
        "either `pipeline_id` or `ref`, not both",
    );
    assert_match(
        &t.refused(
            "workflow_status",
            json!({ "repo": "g/r", "pipeline_id": 0 }),
        )
        .await,
        "`pipeline_id` must be between 1",
    );
    assert_match(
        &t.refused("workflow_status", json!({ "repo": "g/r", "ref": "a..b" }))
            .await,
        r"`ref` 'a\.\.b' is not a valid",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "workflow_status",
            json!({ "repo": "g/r", "pipeline_id": 99 }),
        )
        .await,
        r"could not find pipeline 99 in g/r \(404\): 404 Not found\. Find pipeline ids with workflow_list",
    );
}

/* ------------------------------------------------------------------ workflow_logs */

#[tokio::test]
async fn workflow_logs_reads_the_log_of_each_failed_job_cleaned_and_cut_to_its_end() {
    let t = Harness::with_token();
    let trace = format!(
        "section_start:1700000000:step_script\r\u{1b}[0K\u{1b}[36;1mRunning tests\u{1b}[0;m\r\n{}\u{1b}[31;1mERROR: Job failed: exit code 1\u{1b}[0;m\r\n",
        "ok line\r\n".repeat(5000)
    );
    t.mock(vec![
        reply(
            200,
            json!([
                job(json!({})),
                job(json!({ "id": 9002, "name": "lint", "status": "success" })),
                job(json!({ "id": 9004, "name": "build", "failure_reason": "runner_system_failure" })),
            ]),
        ),
        text(&trace),
        text("building\nERROR: runner went away\n"),
    ]);
    let payload = t
        .ok(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501 }),
        )
        .await;
    let calls = t.calls();
    assert_eq!(
        calls[0].url.split('?').next().unwrap(),
        format!("{}/pipelines/501/jobs", project())
    );
    assert_eq!(calls[1].url, format!("{}/jobs/9001/trace", project()));
    assert_eq!(calls[2].url, format!("{}/jobs/9004/trace", project()));
    assert_eq!(payload["job_count"], json!(2));
    let jobs = payload["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(
        jobs.iter()
            .map(|j| j["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["unit test", "build"]
    );
    let first = jobs[0]["log"].as_str().unwrap();
    assert_eq!(jobs[0]["truncated"], json!(true));
    assert!(first.starts_with("(truncated) ..."));
    assert!(
        first.ends_with("ERROR: Job failed: exit code 1"),
        "the end, where the failure is, is kept"
    );
    assert!(first.chars().count() <= 10_000 + 20);
    assert_no_match(first, "\u{1b}|section_start|\r");
    assert_eq!(jobs[1]["truncated"], json!(false));
    assert_eq!(jobs[1]["log"], json!("building\nERROR: runner went away"));
    assert_eq!(payload["truncated"], json!(true));
    assert_match(
        summary(&payload),
        r"^Logs of 2 failed jobs in pipeline 501 of g/r\. A long log is cut to its last 10000 characters\.$",
    );
}

#[tokio::test]
async fn workflow_logs_reads_one_job_by_id_or_every_job_when_asked() {
    let t = Harness::with_token();
    let list = json!([
        job(json!({})),
        job(json!({ "id": 9002, "name": "lint", "status": "success", "failure_reason": null }))
    ]);
    t.mock(vec![
        reply(200, list.clone()),
        reply(200, json!("all good\n")),
        reply(200, list),
        reply(200, json!("first\n")),
        reply(200, json!("second\n")),
    ]);
    let one = t
        .ok(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501, "job_id": 9002 }),
        )
        .await;
    assert_eq!(t.calls()[1].url, format!("{}/jobs/9002/trace", project()));
    assert_eq!(
        summary(&one),
        "Log of job 'lint' (9002) in pipeline 501 of g/r."
    );
    assert_eq!(one["jobs"][0]["log"], json!("all good"));
    let every = t
        .ok(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501, "only_failed": false }),
        )
        .await;
    assert_eq!(every["jobs"].as_array().unwrap().len(), 2);
    assert_match(summary(&every), "^Logs of 2 jobs in pipeline 501");
}

#[tokio::test]
async fn workflow_logs_says_when_there_is_nothing_to_read_and_why_a_job_has_no_log() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            json!([
                job(json!({ "status": "success", "failure_reason": null })),
                job(json!({ "id": 9005, "name": "deploy", "status": "running", "failure_reason": null })),
            ]),
        ),
        reply(200, json!([])),
        reply(200, json!([job(json!({}))])),
        reply(404, json!({ "message": "404 Not Found" })),
        reply(200, json!([job(json!({}))])),
        reply(200, json!("\n\n")),
    ]);
    let args = json!({ "repo": "g/r", "pipeline_id": 501 });
    let none = t.ok("workflow_logs", args.clone()).await;
    assert_eq!(none["job_count"], json!(0));
    assert_match(
        summary(&none),
        "has no failed jobs so far, and some jobs have not finished\\. Pass only_failed false",
    );
    let empty = t.ok("workflow_logs", args.clone()).await;
    assert_eq!(summary(&empty), "Pipeline 501 in g/r has no jobs yet.");
    let erased = t.ok("workflow_logs", args.clone()).await;
    assert_match(
        erased["jobs"][0]["note"].as_str().unwrap(),
        "GitLab has no log for this job",
    );
    assert!(erased["jobs"][0].get("log").is_none());
    let blank = t.ok("workflow_logs", args).await;
    assert_eq!(blank["jobs"][0]["note"], json!("The log is empty."));
}

#[tokio::test]
async fn workflow_logs_reads_at_most_five_jobs_and_refuses_a_job_of_another_pipeline_and_bad_arguments()
 {
    let t = Harness::with_token();
    let many: Vec<Value> = (0..7)
        .map(|i| job(json!({ "id": 100 + i, "name": format!("shard {i}") })))
        .collect();
    let mut answers = vec![reply(200, json!(many))];
    answers.extend((0..5).map(|i| reply(200, json!(format!("log {i}\n")))));
    answers.push(reply(200, json!([job(json!({}))])));
    answers.push(reply(404, json!({ "message": "404 Not found" })));
    t.mock(answers);
    assert_match(
        &t.refused("workflow_logs", json!({ "repo": "g/r" })).await,
        "`pipeline_id` is required",
    );
    assert_match(
        &t.refused(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501, "job_id": 0 }),
        )
        .await,
        "`job_id` must be between 1",
    );
    assert_match(
        &t.refused(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501, "only_failed": "no" }),
        )
        .await,
        "`only_failed` must be true or false",
    );
    assert_eq!(t.call_count(), 0);
    let payload = t
        .ok(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501 }),
        )
        .await;
    assert_eq!(payload["jobs"].as_array().unwrap().len(), 5);
    assert_eq!(payload["job_count"], json!(7));
    assert_eq!(payload["truncated"], json!(true));
    assert_match(
        summary(&payload),
        r"Showing the first 5 of 7; pass job_id for the others\.",
    );
    assert_match(
        &t.refused(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 501, "job_id": 77 }),
        )
        .await,
        "Job 77 is not among the 1 jobs listed for pipeline 501 in g/r",
    );
    assert_match(
        &t.refused(
            "workflow_logs",
            json!({ "repo": "g/r", "pipeline_id": 999 }),
        )
        .await,
        r"could not find pipeline 999 in g/r \(404\): 404 Not found\. Find pipeline ids with workflow_status or workflow_list",
    );
}

/* ------------------------------------------------------------------ clean_log */

#[test]
fn clean_log_drops_colour_codes_section_markers_the_byte_order_mark_and_carriage_returns() {
    let raw = "\u{feff}section_start:1700000000:prepare_executor\r\u{1b}[0K\u{1b}[36;1mPreparing\u{1b}[0;m\r\n\
               section_start:1700000001:step_script[collapsed=true]\r\u{1b}[0Kline\r\n\
               section_end:1700000002:step_script\r\u{1b}[0K\n\
               \u{1b}[31;1mERROR: Job failed\u{1b}[0;m\n";
    assert_eq!(clean_log(raw), "Preparing\nline\n\nERROR: Job failed\n");
    assert_eq!(clean_log("plain\n"), "plain\n");
}
