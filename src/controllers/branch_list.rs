//! List a project's branches.
use super::prelude::*;

pub fn def() -> ToolDef {
    let mut props = vec![
        repo_arg(),
        opt("search", string_arg("Only branches whose name contains this; '^feat' matches the start, 'fix$' the end.")),
    ];
    props.extend(paging_args());
    let mut out = vec![req(
        "items",
        array(obj(vec![
            req("name", string()),
            req("sha", string()),
            req("protected", boolean()),
            req("default", boolean()),
            req("merged", boolean()),
        ])),
    )];
    out.extend(paging_out());
    ToolDef {
        name: "branch_list",
        description: "List the branches of a GitLab project by name, with the sha at each tip and whether it is \
                      protected, the default or already merged. Paged. Use it to see what branches exist before \
                      creating, merging or deleting one.",
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
        &format!("{}/repository/branches", repo.path),
        Call::new(format!("list the branches of {}", repo.full)).query("search", search.as_ref()).queries(page.query()),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|branch| {
            Fields::new()
                .raw("name", at(branch, "name"))
                .set("sha", or(at(branch, "commit.id"), &json!("")).clone())
                .set("protected", truthy(at(branch, "protected")))
                .set("default", truthy(at(branch, "default")))
                .set("merged", truthy(at(branch, "merged")))
                .into_value()
        })
        .collect();
    let matching = search.map(|s| format!(" matching '{s}'")).unwrap_or_default();
    let summary = if items.is_empty() {
        format!("{} has no branches{matching} on page {}.", repo.full, page.page)
    } else {
        format!("{} {}{matching} of {} on page {}.", items.len(), plural(items.len(), "branch", "branches"), repo.full, page.page)
    };
    Ok(Outcome::ok(summary, paging(Fields::new().set("items", items), page, &answer.headers)))
}
