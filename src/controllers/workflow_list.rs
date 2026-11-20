//! List a project's recent pipelines.
use super::prelude::*;
use crate::utils::js::parse_int;

const STATUSES: &[&str] = &[
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

pub fn def() -> ToolDef {
    let mut props = vec![
        repo_arg(),
        opt("ref", string_arg("Only pipelines on this branch or tag.")),
        opt("status", enum_arg(STATUSES, "Only pipelines in this status, e.g. 'failed' or 'running'.")),
    ];
    props.extend(paging_args());
    let mut out = vec![
        req(
            "items",
            array(obj(vec![
                req("id", num()),
                req("status", string()),
                req("ref", string()),
                req("sha", string()),
                opt("source", string()),
                opt("created_at", string()),
                req("html_url", string()),
            ])),
        ),
        opt("total_count", num()),
    ];
    out.extend(paging_out());
    ToolDef {
        name: "workflow_list",
        description: "List the recent CI/CD pipelines of a GitLab project, newest first, with their id, status, branch or tag, \
                      commit and what started them. Paged. A project has one pipeline definition, its .gitlab-ci.yml, so this \
                      lists runs and not workflows. Use it to find the pipeline to follow with workflow_status.",
        input_schema: input(props),
        output_schema: output(out),
        annotations: READ,
    }
}

fn or_empty(value: &Value) -> Value {
    if value.is_null() { json!("") } else { value.clone() }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let reference = match arg(args, "ref") {
        Value::Null => None,
        value => Some(ref_name(value, "ref")?),
    };
    let status = one_of(arg(args, "status"), "status", STATUSES, None)?;
    let page = page_query(args)?;

    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/pipelines", repo.path),
        Call::new(format!("list the pipelines of {}", repo.full))
            .query("ref", reference.as_ref())
            .query("status", status.as_ref())
            .query("order_by", Some("id"))
            .query("sort", Some("desc"))
            .queries(page.query()),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|pipeline| {
            Fields::new()
                .raw("id", at(pipeline, "id"))
                .raw("status", at(pipeline, "status"))
                .set("ref", or_empty(at(pipeline, "ref")))
                .set("sha", or_empty(at(pipeline, "sha")))
                .truthy("source", at(pipeline, "source"))
                .truthy("created_at", at(pipeline, "created_at"))
                .raw("html_url", at(pipeline, "web_url"))
                .into_value()
        })
        .collect();
    let total = answer.headers.get("x-total").and_then(|t| parse_int(t));
    let narrowed = format!(
        "{}{}",
        reference.as_ref().map(|r| format!(" on {r}")).unwrap_or_default(),
        status.as_ref().map(|s| format!(" that are {s}")).unwrap_or_default()
    );

    let summary = if items.is_empty() {
        format!("{} has no pipelines{narrowed} on page {}.", repo.full, page.page)
    } else {
        format!(
            "{} {}{narrowed} of {}, newest first, on page {}{}",
            items.len(),
            plural(items.len(), "pipeline", "pipelines"),
            repo.full,
            page.page,
            total.map(|t| format!(" of {t} in all.")).unwrap_or_else(|| ".".into())
        )
    };
    Ok(Outcome::ok(
        summary,
        paging(Fields::new().set("items", items).opt("total_count", total), page, &answer.headers),
    ))
}
