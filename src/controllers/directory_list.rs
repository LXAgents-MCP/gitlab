//! List what is in a directory of a repository.
use super::prelude::*;

pub fn def() -> ToolDef {
    let mut props = vec![
        repo_arg(),
        opt("path", string_arg("Directory inside the repository, e.g. 'src/tools'. Omit for the root.")),
        opt("ref", string_arg("Branch, tag or commit sha. Default: the default branch.")),
    ];
    props.extend(paging_args());
    let mut out = vec![
        req("path", string()),
        req("items", array(obj(vec![req("name", string()), req("type", string()), req("path", string())]))),
    ];
    out.extend(paging_out());
    ToolDef {
        name: "directory_list",
        description: "List the files and folders in one directory of a GitLab project, on the default branch or at a \
                      branch, tag or commit. Not recursive. Paged. Omit the path for the repository root. Use it to find \
                      a path before file_read.",
        input_schema: input(props),
        output_schema: output(out),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let dir = file_path(arg(args, "path"), "path", true)?;
    let reference = optional_ref(arg(args, "ref"), "ref")?;
    let page = page_query(args)?;
    let where_ = if dir.path.is_empty() { "the root".to_string() } else { format!("'{}'", dir.path) };
    let at_ref = reference.as_ref().map(|r| format!(" at {r}")).unwrap_or_default();

    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/tree", repo.path),
        Call::new(format!("list {where_} of {}", repo.full))
            .query("path", Some(&dir.path))
            .query("ref", reference.as_ref())
            .queries(page.query())
            .explain(|status, detail| {
                (status == 404).then(|| {
                    format!(
                        "There is no directory {where_} in {}{at_ref} (404){detail}. Check the path and the ref; a file \
                         cannot be listed. GitLab also answers 404 for a private project the token cannot see.",
                        repo.full
                    )
                })
            }),
    )
    .await?;

    let items: Vec<Value> = list(&answer.body)
        .iter()
        .map(|entry| {
            // GitLab's names for what a tree holds, in the words a path is spoken in.
            let kind = match at(entry, "type").as_str() {
                Some("tree") => Value::from("dir"),
                Some("blob") => Value::from("file"),
                Some("commit") => Value::from("submodule"),
                _ => match at(entry, "type") {
                    Value::Null => Value::from("unknown"),
                    other => Value::from(js_string(other)),
                },
            };
            Fields::new()
                .raw("name", at(entry, "name"))
                .set("type", kind)
                .set("path", or(at(entry, "path"), at(entry, "name")).clone())
                .into_value()
        })
        .collect();
    let summary = if items.is_empty() {
        format!("No entries in {where_} of {}{at_ref} on page {}.", repo.full, page.page)
    } else {
        format!(
            "{} {} in {where_} of {}{at_ref}, on page {}.",
            items.len(),
            plural(items.len(), "entry", "entries"),
            repo.full,
            page.page
        )
    };
    Ok(Outcome::ok(summary, paging(Fields::new().set("path", dir.path.clone()).set("items", items), page, &answer.headers)))
}
