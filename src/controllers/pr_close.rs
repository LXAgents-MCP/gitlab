//! Close a merge request without merging it.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_close",
        description: "Close a merge request in a GitLab project without merging it, optionally leaving a comment first. \
                      Its branch is kept. A merge request that is already merged cannot be closed. There is no tool to reopen it.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Merge request number in the project.")),
            opt("comment", string_arg("A comment to post before closing, e.g. why.")),
        ]),
        output_schema: output(vec![
            req("number", num()),
            req("state", string()),
            req("merged", boolean()),
            req("html_url", string()),
        ]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let comment = raw_text(arg(args, "comment"), "comment")?;
    let commented = comment.as_ref().is_some_and(|c| !c.trim().is_empty());
    let path = format!("{}/merge_requests/{number}", repo.path);

    if commented {
        gitlab(
            ctx,
            "POST",
            &format!("{path}/notes"),
            Call::new(format!("comment on merge request !{number} of {} before closing it", repo.full))
                .json(json!({ "body": comment })),
        )
        .await?;
    }

    let closed = match gitlab(
        ctx,
        "PUT",
        &path,
        Call::new(format!("close merge request !{number} of {}", repo.full)).json(json!({ "state_event": "close" })).explain(
            |status, detail| {
                (status == 405 || status == 422).then(|| {
                    format!(
                        "GitLab would not close merge request !{number} of {} ({status}){detail}. A merge request that \
                         is already merged cannot be closed; read it with pr_read.",
                        repo.full
                    )
                })
            },
        ),
    )
    .await
    {
        Ok(answer) => answer.body,
        Err(refused) if commented => return Err(Refused(format!("{} The comment had already been posted.", refused.0))),
        Err(refused) => return Err(refused),
    };

    expect_body(&closed, "web_url", "a merge request")?;
    let state = at(&closed, "state");
    Ok(Outcome::ok(
        if state.as_str() == Some("closed") {
            format!("Closed merge request !{number} in {} without merging.", repo.full)
        } else {
            format!("GitLab answered, but merge request !{number} in {} is {}.", repo.full, js_string(state))
        },
        Fields::new()
            .set("number", number)
            .raw("state", state)
            .set("merged", state.as_str() == Some("merged"))
            .raw("html_url", at(&closed, "web_url")),
    ))
}
