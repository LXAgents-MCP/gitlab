//! Start a CI/CD pipeline.
use std::sync::LazyLock;

use regex::Regex;

use super::prelude::*;
use crate::utils::js::{js_entries, js_number};

static VARIABLE_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_]{1,255}$").expect("pattern"));

/// The variables of a new pipeline. GitLab takes every value as text, as a job reads it from its
/// environment, so a number or a flag is written out and checked by the job, not here.
fn pipeline_variables(value: &Value) -> Result<Option<Vec<Value>>, Refused> {
    let object = match value {
        Value::Null => return Ok(None),
        Value::Object(object) => object,
        _ => return Err(Refused::new("`variables` must be an object of variable names and their values.")),
    };
    let entries = js_entries(object);
    if entries.len() > MAX_PIPELINE_VARIABLES {
        return Err(Refused(format!(
            "`variables` may hold at most {MAX_PIPELINE_VARIABLES} variables, got {}.",
            entries.len()
        )));
    }
    let mut variables = Vec::new();
    for (key, entry) in entries {
        if !VARIABLE_NAME.is_match(key) {
            return Err(Refused(format!(
                "`variables` has a name '{key}' that is not letters, digits and underscores, 1 to 255 long."
            )));
        }
        let text = match entry {
            Value::String(text) => text.clone(),
            Value::Number(number) => js_number(number),
            Value::Bool(flag) => flag.to_string(),
            _ => return Err(Refused(format!("`variables.{key}` must be a string, a number or true/false."))),
        };
        variables.push(json!({ "key": key, "value": text }));
    }
    Ok((!variables.is_empty()).then_some(variables))
}

pub fn def() -> ToolDef {
    ToolDef {
        name: "workflow_run",
        description: "Start a CI/CD pipeline of a GitLab project on a branch or tag (default: the default branch), with optional \
                      variables. It runs whatever .gitlab-ci.yml does on that ref, which may deploy or publish. The ref's \
                      .gitlab-ci.yml must allow a pipeline started through the API. Returns the pipeline id; follow it with \
                      workflow_status.",
        input_schema: input(vec![
            repo_arg(),
            opt("ref", string_arg("Branch or tag to run it on. Default: the project's default branch.")),
            opt(
                "variables",
                json!({
                    "description": "CI/CD variables for this pipeline by name, up to 25. Values are sent as text and shown in \
                                    the pipeline, so never a secret.",
                    "type": "object",
                    "propertyNames": { "type": "string" },
                    "additionalProperties": { "type": ["string", "number", "boolean"] },
                }),
            ),
        ]),
        output_schema: output(vec![
            req("pipeline_id", num()),
            req("status", string()),
            req("ref", string()),
            opt("sha", string()),
            req("html_url", string()),
        ]),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let given = match arg(args, "ref") {
        Value::Null => None,
        value => Some(ref_name(value, "ref")?),
    };
    let variables = pipeline_variables(arg(args, "variables"))?;

    let reference = branch_or_default(ctx, &repo, given).await?;
    let mut request = json!({ "ref": reference });
    if let Some(variables) = variables {
        request["variables"] = Value::Array(variables);
    }
    let created = gitlab(
        ctx,
        "POST",
        &format!("{}/pipeline", repo.path),
        Call::new(format!("start a pipeline on '{reference}' in {}", repo.full)).json(request).explain(|status, detail| {
            (status == 400).then(|| {
                format!(
                    "GitLab would not start a pipeline on '{reference}' in {} (400){detail}. The ref must exist, its \
                     .gitlab-ci.yml must be valid and give the pipeline at least one job, and the project's workflow rules must \
                     allow a pipeline started through the API. Setting variables can also be limited to a higher role.",
                    repo.full
                )
            })
        }),
    )
    .await?
    .body;

    expect_body(&created, "web_url", "a pipeline")?;
    Ok(Outcome::ok(
        format!(
            "Started pipeline {} on {reference} in {}; it is {}.",
            js_string(at(&created, "id")),
            repo.full,
            js_string(at(&created, "status"))
        ),
        Fields::new()
            .raw("pipeline_id", at(&created, "id"))
            .raw("status", at(&created, "status"))
            .set("ref", reference.clone())
            .truthy("sha", at(&created, "sha"))
            .raw("html_url", at(&created, "web_url")),
    ))
}
