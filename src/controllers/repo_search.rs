//! Search projects across GitLab.
use super::prelude::*;
use crate::utils::js::parse_int;

const SORTS: &[&str] = &["name", "path", "created_at", "updated_at", "last_activity_at", "star_count", "similarity"];
const ORDERS: &[&str] = &["asc", "desc"];

pub fn def() -> ToolDef {
    let mut props = vec![
        req("query", string_arg("Words to look for in a project's name, path or description, e.g. 'mcp server'.")),
        opt(
            "sort",
            enum_arg(
                SORTS,
                "name, path, created_at, updated_at, last_activity_at, star_count or similarity. Default: newest first.",
            ),
        ),
        opt("order", enum_arg(ORDERS, "asc or desc (default). Only applies when sort is given.")),
    ];
    props.extend(paging_args());
    let mut out = vec![
        req(
            "items",
            array(obj(vec![
                req("full_name", string()),
                opt("description", string()),
                req("stars", num()),
                opt("last_activity_at", string()),
                req("url", string()),
            ])),
        ),
        req("total_count", num()),
    ];
    out.extend(paging_out());
    ToolDef {
        name: "repo_search",
        description: "Search GitLab projects by words in their name, path or description, such as 'mcp server'; several \
                      words must all match. Use it to find a repository when its group is not known. It sees the \
                      projects the token can see; similarity ranking only covers projects the token's user belongs to.",
        input_schema: input(props),
        output_schema: output(out),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let query = required_text(arg(args, "query"), "query")?;
    let sort = one_of(arg(args, "sort"), "sort", SORTS, None)?;
    let order = one_of(arg(args, "order"), "order", ORDERS, None)?;
    let page = page_query(args)?;

    let answer = gitlab(
        ctx,
        "GET",
        "/projects",
        Call::new(format!("search projects for '{query}'"))
            .query("search", Some(&query))
            .query("order_by", sort.as_ref())
            .query("sort", if sort.as_deref().is_some_and(|s| !s.is_empty()) { order.as_ref() } else { None })
            .queries(page.query()),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|project| {
            Fields::new()
                .raw("full_name", at(project, "path_with_namespace"))
                .truthy("description", at(project, "description"))
                .set("stars", if at(project, "star_count").is_null() { json!(0) } else { at(project, "star_count").clone() })
                .truthy("last_activity_at", at(project, "last_activity_at"))
                .raw("url", at(project, "web_url"))
                .into_value()
        })
        .collect();
    // GitLab leaves the total out of an answer with more than 10,000 matches.
    let total = answer
        .headers
        .get("x-total")
        .and_then(|t| parse_int(t))
        .filter(|n| *n >= 0)
        .unwrap_or(items.len() as i64);

    let summary = if items.is_empty() {
        format!("No projects match '{query}'.")
    } else {
        format!(
            "{total} {} '{query}'; showing {}.",
            if total == 1 { "project matches" } else { "projects match" },
            items.len()
        )
    };
    Ok(Outcome::ok(
        summary,
        paging(Fields::new().set("items", items).set("total_count", total), page, &answer.headers),
    ))
}
