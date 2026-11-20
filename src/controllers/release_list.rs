//! List a project's releases.
use super::prelude::*;

pub fn def() -> ToolDef {
    let mut props = vec![repo_arg()];
    props.extend(paging_args());
    let mut out = vec![req(
        "items",
        array(obj(vec![
            req("tag", string()),
            opt("name", string()),
            req("upcoming", boolean()),
            opt("released_at", string()),
        ])),
    )];
    out.extend(paging_out());
    ToolDef {
        name: "release_list",
        description: "List the releases of a GitLab project, newest release date first, upcoming ones included. Paged. \
                      Use release_read for one release's notes and assets.",
        input_schema: input(props),
        output_schema: output(out),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let page = page_query(args)?;

    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/releases", repo.path),
        Call::new(format!("list the releases of {}", repo.full))
            .query("order_by", Some("released_at"))
            .query("sort", Some("desc"))
            .queries(page.query()),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|release| {
            Fields::new()
                .raw("tag", at(release, "tag_name"))
                .truthy("name", at(release, "name"))
                .set("upcoming", truthy(at(release, "upcoming_release")))
                .truthy("released_at", at(release, "released_at"))
                .into_value()
        })
        .collect();
    let summary = if items.is_empty() {
        format!("{} has no releases on page {}.", repo.full, page.page)
    } else {
        format!("{} {} of {} on page {}.", items.len(), plural(items.len(), "release", "releases"), repo.full, page.page)
    };
    Ok(Outcome::ok(summary, paging(Fields::new().set("items", items), page, &answer.headers)))
}
