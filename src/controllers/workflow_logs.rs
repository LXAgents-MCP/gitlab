//! Read the logs of the jobs of a CI/CD pipeline.
use super::prelude::*;
use crate::models::shapes::{job_fields, job_shape};
use crate::utils::text::{clean_log, tail};

const FINISHED: &[&str] = &["success", "failed", "canceled", "skipped", "manual"];

fn is_failed(job: &Value) -> bool {
    at(job, "status").as_str() == Some("failed")
}

pub fn def() -> ToolDef {
    let mut job = job_shape();
    job.push(opt("log", string()));
    job.push(req("truncated", boolean()));
    job.push(opt("note", string()));
    ToolDef {
        name: "workflow_logs",
        description: "Read the logs of the jobs of a GitLab CI/CD pipeline. By default only the failed jobs, up to 5, each cut to its \
                      last lines, where a failure is explained; job_id reads one job and only_failed false reads every job. At most \
                      20,000 characters in all. Find pipeline and job ids with workflow_status.",
        input_schema: input(vec![
            repo_arg(),
            req("pipeline_id", int_arg("Pipeline id.")),
            opt("job_id", int_arg("Read just this job of the pipeline. Default: the failed jobs.")),
            opt(
                "only_failed",
                bool_arg("false reads every job of the pipeline, not only the failed ones. Default true. Ignored with job_id."),
            ),
        ]),
        output_schema: output(vec![
            req("pipeline_id", num()),
            req("job_count", num()),
            req("jobs", array(obj(job))),
            req("truncated", boolean()),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let pipeline_id = required_int(arg(args, "pipeline_id"), "pipeline_id", IntRange::min(1))?;
    let job_id = optional_int(arg(args, "job_id"), "job_id", IntRange::min(1))?;
    let only_failed = optional_flag(arg(args, "only_failed"), "only_failed", true)?;

    let listing = gitlab(
        ctx,
        "GET",
        &format!("{}/pipelines/{pipeline_id}/jobs", repo.path),
        Call::new(format!("list the jobs of pipeline {pipeline_id} of {}", repo.full))
            .query("per_page", Some(MAX_PER_PAGE))
            .explain(|status, detail| {
                (status == 404).then(|| {
                    format!(
                        "GitLab could not find pipeline {pipeline_id} in {} (404){detail}. Find pipeline ids with \
                         workflow_status or workflow_list. GitLab also answers 404 for a private project the token cannot see.",
                        repo.full
                    )
                })
            }),
    )
    .await?
    .body;
    let all = list(&listing);

    let selected: Vec<&Value> = match job_id {
        Some(job_id) => {
            let found: Vec<&Value> = all.iter().filter(|job| as_int(at(job, "id")) == Some(job_id)).collect();
            if found.is_empty() {
                return Err(Refused(format!(
                    "Job {job_id} is not among the {} jobs listed for pipeline {pipeline_id} in {}. \
                     Read the pipeline with workflow_status to see its jobs.",
                    all.len(),
                    repo.full
                )));
            }
            found
        }
        None if only_failed => all.iter().filter(|job| is_failed(job)).collect(),
        None => all.iter().collect(),
    };

    if selected.is_empty() {
        let unfinished = all.iter().any(|job| !at(job, "status").as_str().is_some_and(|s| FINISHED.contains(&s)));
        let summary = if all.is_empty() {
            format!("Pipeline {pipeline_id} in {} has no jobs yet.", repo.full)
        } else {
            format!(
                "Pipeline {pipeline_id} in {} has no failed jobs{}. Pass only_failed false to read every job, or job_id for one.",
                repo.full,
                if unfinished { " so far, and some jobs have not finished" } else { "" }
            )
        };
        return Ok(Outcome::ok(
            summary,
            Fields::new()
                .set("pipeline_id", pipeline_id)
                .set("job_count", 0)
                .set("jobs", Value::Array(vec![]))
                .set("truncated", false),
        ));
    }

    // The jobs share the budget, one request each and one after another, so a matrix of failures
    // is not a burst against the rate limit.
    let reading = &selected[..selected.len().min(MAX_LOG_JOBS)];
    let share = MAX_LOG_CHARS / reading.len();
    let mut jobs: Vec<Fields> = Vec::new();
    let mut cut = selected.len() > reading.len();

    for job in reading {
        let fields = job_fields(job);
        let id = js_string(at(job, "id"));
        let answer = gitlab(
            ctx,
            "GET",
            &format!("{}/jobs/{id}/trace", repo.path),
            Call::new(format!("read the log of job {id} of {}", repo.full)).raw().allow(&[404, 410]),
        )
        .await?;

        if answer.status == 404 || answer.status == 410 {
            jobs.push(fields.set("truncated", false).set(
                "note",
                "GitLab has no log for this job: it may not have started, or the log has been erased.",
            ));
            continue;
        }
        let cleaned = clean_log(answer.body.as_str().unwrap_or(""));
        let text = cleaned.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        if text.is_empty() {
            jobs.push(fields.set("truncated", false).set("note", "The log is empty."));
            continue;
        }
        let (log, shortened) = tail(text, share);
        cut = cut || shortened;
        jobs.push(fields.set("log", log).set("truncated", shortened));
    }

    let summary = match job_id {
        Some(job_id) => format!(
            "Log of job '{}' ({job_id}) in pipeline {pipeline_id} of {}.",
            jobs[0].get("name").map(js_string).unwrap_or_default(),
            repo.full
        ),
        None => {
            let more = if selected.len() > reading.len() {
                format!(" Showing the first {} of {}; pass job_id for the others.", reading.len(), selected.len())
            } else {
                String::new()
            };
            format!(
                "Logs of {} {}{} in pipeline {pipeline_id} of {}.{more}",
                jobs.len(),
                if only_failed { "failed " } else { "" },
                plural(jobs.len(), "job", "jobs"),
                repo.full
            )
        }
    };
    let any_cut = jobs.iter().any(|job| job.get("truncated").is_some_and(truthy));
    let summary = if any_cut { format!("{summary} A long log is cut to its last {share} characters.") } else { summary };
    Ok(Outcome::ok(
        summary,
        Fields::new()
            .set("pipeline_id", pipeline_id)
            .set("job_count", selected.len())
            .set("jobs", jobs.into_iter().map(Fields::into_value).collect::<Vec<_>>())
            .set("truncated", cut),
    ))
}
