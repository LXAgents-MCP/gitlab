//! List a project's tags.
use super::prelude::*;

pub fn def() -> ToolDef {
    let mut props = vec![
        repo_arg(),
        opt("search", string_arg("Only tags whose name contains this; '^v1' matches the start, 'rc$' the end.")),
    ];
    props.extend(paging_args());
    let mut out = vec![req("items", array(obj(vec![req("name", string()), req("sha", string())])))];
    out.extend(paging_out());
    ToolDef {
        name: "tag_get_all",
        description: "List the tags of a GitLab project, most recently updated first, with the commit each points at. \
                      Paged. Use it to see what versions exist, or to find a tag before reading or releasing it.",
        input_schema: input(props),
        output_schema: output(out),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let search = optional_text(arg(args, "search"), "search")?;
    let page = page_query(args)?;

    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/tags", repo.path),
        Call::new(format!("list the tags of {}", repo.full)).query("search", search.as_ref()).queries(page.query()),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|tag| {
            let sha = at(tag, "commit.id");
            Fields::new()
                .raw("name", at(tag, "name"))
                .set("sha", if sha.is_null() { json!("") } else { sha.clone() })
                .into_value()
        })
        .collect();
    let matching = search.map(|s| format!(" matching '{s}'")).unwrap_or_default();
    let summary = if items.is_empty() {
        format!("{} has no tags{matching} on page {}.", repo.full, page.page)
    } else {
        format!("{} {}{matching} of {} on page {}.", items.len(), plural(items.len(), "tag", "tags"), repo.full, page.page)
    };
    Ok(Outcome::ok(summary, paging(Fields::new().set("items", items), page, &answer.headers)))
}
