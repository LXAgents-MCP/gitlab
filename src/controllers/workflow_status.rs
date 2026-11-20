//! Read the state of a CI/CD pipeline and its jobs.
use super::prelude::*;
use crate::models::shapes::{job_fields, job_shape};
use crate::utils::js::parse_int;

pub fn def() -> ToolDef {
    ToolDef {
        name: "workflow_status",
        description: "Read a GitLab CI/CD pipeline: its status, branch or tag, commit, timing, and each job's result with the \
                      reason a failed one failed. Give a pipeline_id, or a ref to read the latest pipeline on that branch or tag. \
                      After workflow_run, ask again until the status is success, failed, canceled or skipped.",
        input_schema: input(vec![
            repo_arg(),
            opt("pipeline_id", int_arg("Pipeline id, from workflow_run, workflow_list or an earlier workflow_status.")),
            opt("ref", string_arg("Branch or tag; reads its most recent pipeline.")),
        ]),
        output_schema: output(vec![
            req("found", boolean()),
            opt("pipeline_id", num()),
            opt("name", string()),
            opt("status", string()),
            opt("ref", string()),
            opt("sha", string()),
            opt("source", string()),
            opt("user", string()),
            opt("created_at", string()),
            opt("started_at", string()),
            opt("finished_at", string()),
            opt("duration", num()),
            opt("yaml_errors", string()),
            opt("html_url", string()),
            opt("job_count", num()),
            req("jobs", array(obj(job_shape()))),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let pipeline_id = optional_int(arg(args, "pipeline_id"), "pipeline_id", IntRange::min(1))?;
    let reference = match optional_text(arg(args, "ref"), "ref")? {
        None => None,
        Some(_) => Some(ref_name(arg(args, "ref"), "ref")?),
    };

    if pipeline_id.is_some() && reference.is_some() {
        return Err(Refused::new("Give either `pipeline_id` or `ref`, not both."));
    }
    let id: Value = match (pipeline_id, &reference) {
        (Some(id), _) => Value::from(id),
        (None, None) => {
            return Err(Refused::new("Give `pipeline_id`, or `ref` to read the latest pipeline on a branch or tag."));
        }
        (None, Some(reference)) => {
            // Listing answers an empty list for a ref with no pipeline, where asking for "the latest" answers 403 or 404
            // that cannot be told from a project that cannot be seen.
            let body = gitlab(
                ctx,
                "GET",
                &format!("{}/pipelines", repo.path),
                Call::new(format!("find the latest pipeline on '{reference}' in {}", repo.full))
                    .query("ref", Some(reference))
                    .query("order_by", Some("id"))
                    .query("sort", Some("desc"))
                    .query("per_page", Some(1)),
            )
            .await?
            .body;
            let first = list(&body).first().map(|p| at(p, "id").clone()).unwrap_or(Value::Null);
            if !is_int(&first) {
                return Ok(Outcome::ok(
                    format!(
                        "No pipeline on {reference} in {} yet. A pipeline appears a few seconds after it is triggered; \
                         ask again shortly.",
                        repo.full
                    ),
                    Fields::new().set("found", false).set("jobs", Value::Array(vec![])),
                ));
            }
            first
        }
    };
    let id = js_string(&id);

    let pipeline = gitlab(
        ctx,
        "GET",
        &format!("{}/pipelines/{id}", repo.path),
        Call::new(format!("read pipeline {id} of {}", repo.full)).explain(|status, detail| {
            (status == 404).then(|| {
                format!(
                    "GitLab could not find pipeline {id} in {} (404){detail}. Find pipeline ids with workflow_list. \
                     GitLab also answers 404 for a private project the token cannot see.",
                    repo.full
                )
            })
        }),
    )
    .await?
    .body;
    expect_body(&pipeline, "web_url", "a pipeline")?;

    let listing = gitlab(
        ctx,
        "GET",
        &format!("{}/pipelines/{id}/jobs", repo.path),
        Call::new(format!("list the jobs of pipeline {id} of {}", repo.full)).query("per_page", Some(MAX_PER_PAGE)),
    )
    .await?;
    let jobs: Vec<Fields> = list(&listing.body).iter().map(job_fields).collect();
    let total = listing.headers.get("x-total").and_then(|t| parse_int(t));
    let failed: Vec<String> = jobs
        .iter()
        .filter(|job| {
            job.get("status").and_then(Value::as_str) == Some("failed") && !job.get("allow_failure").is_some_and(truthy)
        })
        .map(|job| job.get("name").map(js_string).unwrap_or_default())
        .collect();

    let name = at(&pipeline, "name").as_str().filter(|n| !n.is_empty());
    let reference = at(&pipeline, "ref");
    let place = if truthy(reference) { format!(" on {}", js_string(reference)) } else { String::new() };
    let mut summary = format!(
        "Pipeline {}{}{place} at {} in {} is {}.",
        js_string(at(&pipeline, "id")),
        name.map(|n| format!(" ({n})")).unwrap_or_default(),
        short(at(&pipeline, "sha")),
        repo.full,
        js_string(at(&pipeline, "status"))
    );
    if !failed.is_empty() {
        summary.push_str(&format!(" Failed: {}.", failed.join(", ")));
    }

    let job_count = total.unwrap_or(jobs.len() as i64);
    let fields = Fields::new()
        .set("found", true)
        .raw("pipeline_id", at(&pipeline, "id"))
        .opt("name", name)
        .raw("status", at(&pipeline, "status"))
        .truthy("ref", reference)
        .truthy("sha", at(&pipeline, "sha"))
        .truthy("source", at(&pipeline, "source"))
        .truthy("user", at(&pipeline, "user.username"))
        .truthy("created_at", at(&pipeline, "created_at"))
        .truthy("started_at", at(&pipeline, "started_at"))
        .truthy("finished_at", at(&pipeline, "finished_at"))
        .opt("duration", at(&pipeline, "duration").is_number().then(|| at(&pipeline, "duration").clone()))
        .opt("yaml_errors", at(&pipeline, "yaml_errors").as_str().filter(|e| !e.is_empty()))
        .raw("html_url", at(&pipeline, "web_url"))
        .set("job_count", job_count)
        .set("jobs", jobs.into_iter().map(Fields::into_value).collect::<Vec<_>>());
    Ok(Outcome::ok(summary, fields))
}
