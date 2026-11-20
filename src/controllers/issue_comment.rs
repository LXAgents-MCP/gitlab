//! Comment on an issue.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_comment",
        description: "Add a comment to an issue of a GitLab project. Visible to everyone who can see the issue. On a \
                      locked issue only project members can comment. For a merge request use pr_comment.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Issue number in the project.")),
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
        &format!("{}/issues/{number}/notes", repo.path),
        Call::new(format!("comment on issue #{number} of {}", repo.full)).json(json!({ "body": body })).explain(
            |status, detail| {
                (status == 403).then(|| {
                    format!(
                        "GitLab would not let this comment on #{number} of {} (403){detail}. The token may lack the `api` \
                         scope, or the discussion may be locked, in which case only project members can comment.",
                        repo.full
                    )
                })
            },
        ),
    )
    .await?
    .body;

    let Some(id) = as_int(at(&created, "id")) else {
        return Err(Refused::new("GitLab answered, but the body was not a comment this server recognizes."));
    };
    // The answer carries no address, so it is built from the path; a project given by id has none.
    let url = repo.full.contains('/').then(|| format!("{WEB_BASE}/{}/-/issues/{number}#note_{id}", repo.full));
    Ok(Outcome::ok(
        format!("Commented on #{number} in {}.", repo.full),
        Fields::new().set("id", at(&created, "id").clone()).opt("html_url", url),
    ))
}
