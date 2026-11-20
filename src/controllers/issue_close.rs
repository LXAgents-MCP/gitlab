//! Close an issue.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_close",
        description: "Close an issue of a GitLab project, optionally leaving a comment first. GitLab has no close reason, \
                      so say why in `comment`. It only closes: there is no tool to reopen. Use pr_close for a merge request.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Issue number in the project.")),
            opt("comment", string_arg("A comment to post before closing, e.g. why.")),
        ]),
        output_schema: output(vec![req("number", num()), req("state", string()), req("html_url", string())]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let comment = raw_text(arg(args, "comment"), "comment")?;
    let commented = comment.as_ref().is_some_and(|c| !c.trim().is_empty());

    if commented {
        gitlab(
            ctx,
            "POST",
            &format!("{}/issues/{number}/notes", repo.path),
            Call::new(format!("comment on issue #{number} of {} before closing it", repo.full)).json(json!({ "body": comment })),
        )
        .await?;
    }

    let closed = match gitlab(
        ctx,
        "PUT",
        &format!("{}/issues/{number}", repo.path),
        Call::new(format!("close issue #{number} of {}", repo.full)).json(json!({ "state_event": "close" })),
    )
    .await
    {
        Ok(answer) => answer.body,
        Err(refused) if commented => return Err(Refused(format!("{} The comment had already been posted.", refused.0))),
        Err(refused) => return Err(refused),
    };

    expect_body(&closed, "web_url", "an issue")?;
    let state = at(&closed, "state");
    Ok(Outcome::ok(
        if state.as_str() == Some("closed") {
            format!("Closed issue #{number} in {}.", repo.full)
        } else {
            format!("GitLab answered, but issue #{number} in {} is still {}.", repo.full, js_string(state))
        },
        Fields::new().set("number", number).raw("state", state).raw("html_url", at(&closed, "web_url")),
    ))
}
