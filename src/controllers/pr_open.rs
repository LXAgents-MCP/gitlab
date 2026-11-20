//! Open a merge request.
use std::sync::LazyLock;

use regex::Regex;

use super::prelude::*;

// GitLab marks a draft by its title; it also reads these spellings, so none is marked twice.
static DRAFT_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(draft:|\[draft\]|\(draft\))").expect("pattern"));

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_open",
        description: "Open a merge request in a GitLab project from a branch of the same project into a base branch \
                      (default: the default branch), optionally as a draft. A draft is a title that starts with 'Draft:', \
                      which this adds. Fails if one is already open for that branch. A branch of a fork is not supported.",
        input_schema: input(vec![
            repo_arg(),
            req("title", string_arg("Merge request title.")),
            req("head", string_arg("Branch with the changes, e.g. 'feat/login'.")),
            opt("base", string_arg("Branch to merge into. Default: the project's default branch.")),
            opt("body", string_arg("Description, in Markdown.")),
            opt("draft", bool_arg("true opens it as a draft. Default false.")),
        ]),
        output_schema: output(vec![
            req("number", num()),
            req("title", string()),
            req("state", string()),
            req("draft", boolean()),
            req("head", string()),
            req("base", string()),
            req("html_url", string()),
        ]),
        annotations: WRITE,
    }
}

/// JavaScript's `a ?? b`.
fn either<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    if a.is_null() { b } else { a }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let title = required_text(arg(args, "title"), "title")?;
    let head = ref_name(arg(args, "head"), "head")?;
    let given = match arg(args, "base") {
        Value::Null => None,
        base => Some(ref_name(base, "base")?),
    };
    let body = raw_text(arg(args, "body"), "body")?;
    let draft = maybe_flag(arg(args, "draft"), "draft")?;

    let base = branch_or_default(ctx, &repo, given).await?;
    let mut request = json!({
        "source_branch": head,
        "target_branch": base,
        "title": if draft == Some(true) && !DRAFT_TITLE.is_match(&title) { format!("Draft: {title}") } else { title.clone() },
    });
    if let Some(body) = body {
        request["description"] = Value::String(body);
    }
    let created = gitlab(
        ctx,
        "POST",
        &format!("{}/merge_requests", repo.path),
        Call::new(format!("open a merge request from '{head}' into '{base}' in {}", repo.full)).json(request).explain(
            |status, detail| {
                (status == 409).then(|| {
                    format!(
                        "GitLab would not open a merge request from '{head}' into '{base}' in {} (409){detail}. \
                         One is already open for that branch; find it with pr_read.",
                        repo.full
                    )
                })
            },
        ),
    )
    .await?
    .body;

    expect_body(&created, "web_url", "a merge request")?;
    let is_draft = truthy(either(at(&created, "draft"), at(&created, "work_in_progress")));
    Ok(Outcome::ok(
        format!(
            "Opened {}merge request !{} in {}: {head} into {base}.",
            if is_draft { "draft " } else { "" },
            js_string(at(&created, "iid")),
            repo.full
        ),
        Fields::new()
            .raw("number", at(&created, "iid"))
            .raw("title", at(&created, "title"))
            .raw("state", at(&created, "state"))
            .set("draft", is_draft)
            .set("head", head.clone())
            .set("base", base.clone())
            .raw("html_url", at(&created, "web_url")),
    ))
}
