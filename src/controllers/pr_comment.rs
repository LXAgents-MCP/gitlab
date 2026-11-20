//! Comment on a merge request's discussion.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_comment",
        description: "Add a comment to a merge request's discussion in a GitLab project. A general comment only: it is \
                      not attached to a line of the diff and is not a review. Visible to everyone who can see the merge request.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Merge request number in the project.")),
            req("body", string_arg("Comment text, in Markdown.")),
        ]),
        output_schema: output(vec![req("id", num()), opt("html_url", string())]),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let body = required_raw_text(arg(args, "body"), "body", false)?;

    let created = gitlab(
        ctx,
        "POST",
        &format!("{}/merge_requests/{number}/notes", repo.path),
        Call::new(format!("comment on merge request !{number} of {}", repo.full)).json(json!({ "body": body })),
    )
    .await?
    .body;

    let Some(id) = as_int(at(&created, "id")) else {
        return Err(Refused::new("GitLab answered, but the body was not a comment this server recognizes."));
    };
    // The answer carries no address, so it is built from the path; a project given by id has none.
    let url = repo.full.contains('/').then(|| format!("{WEB_BASE}/{}/-/merge_requests/{number}#note_{id}", repo.full));
    Ok(Outcome::ok(
        format!("Commented on merge request !{number} in {}.", repo.full),
        Fields::new().set("id", at(&created, "id").clone()).opt("html_url", url),
    ))
}
