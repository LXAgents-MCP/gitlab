//! List the commits of a branch.
use super::prelude::*;

pub fn def() -> ToolDef {
    let mut props = vec![
        repo_arg(),
        opt("ref", string_arg("Branch, tag or commit sha to list back from. Default: the default branch.")),
        opt("path", string_arg("Only commits that touched this file or directory.")),
        opt("author", string_arg("Only commits by this author name or email.")),
        opt("since", string_arg("Only commits after this ISO 8601 date, e.g. 2026-10-01.")),
        opt("until", string_arg("Only commits before this ISO 8601 date.")),
    ];
    props.extend(paging_args());
    let mut out = vec![req(
        "items",
        array(obj(vec![req("sha", string()), req("message", string()), opt("author", string()), opt("date", string())])),
    )];
    out.extend(paging_out());
    ToolDef {
        name: "commit_list",
        description: "List commits of a GitLab project, newest first, from a branch, tag or sha, optionally only \
                      those touching a path, by an author, or within a date range. Paged. Use commit_read for one \
                      commit in full.",
        input_schema: input(props),
        output_schema: output(out),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let reference = optional_ref(arg(args, "ref"), "ref")?;
    let path = Some(file_path(arg(args, "path"), "path", true)?.path).filter(|p| !p.is_empty());
    let author = optional_text(arg(args, "author"), "author")?;
    let since = optional_date(arg(args, "since"), "since")?;
    let until = optional_date(arg(args, "until"), "until")?;
    let page = page_query(args)?;

    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/commits", repo.path),
        Call::new(format!("list the commits of {}", repo.full))
            .query("ref_name", reference.as_ref())
            .query("path", path.as_ref())
            .query("author", author.as_ref())
            .query("since", since.as_ref())
            .query("until", until.as_ref())
            .queries(page.query()),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|commit| {
            Fields::new()
                .raw("sha", at(commit, "id"))
                .set("message", or(at(commit, "title"), &json!("")).clone())
                .truthy("author", at(commit, "author_name"))
                .truthy("date", at(commit, "authored_date"))
                .into_value()
        })
        .collect();
    let on = reference.as_ref().map(|r| format!(" on {r}")).unwrap_or_default();
    let summary = if items.is_empty() {
        format!("No commits{on} of {} match on page {}.", repo.full, page.page)
    } else {
        format!("{} {}{on} of {} on page {}.", items.len(), plural(items.len(), "commit", "commits"), repo.full, page.page)
    };
    Ok(Outcome::ok(summary, paging(Fields::new().set("items", items), page, &answer.headers)))
}
