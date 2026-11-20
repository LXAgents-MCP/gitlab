//! Lock or unlock an issue's discussion.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_lock",
        description: "Lock the discussion of an issue in a GitLab project so only project members can comment, or unlock it. \
                      GitLab takes no reason. Needs permission to edit the issue.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Issue number in the project.")),
            opt("locked", bool_arg("true locks (default), false unlocks.")),
        ]),
        output_schema: output(vec![req("number", num()), req("locked", boolean())]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let lock = optional_flag(arg(args, "locked"), "locked", true)?;

    let answer = gitlab(
        ctx,
        "PUT",
        &format!("{}/issues/{number}", repo.path),
        Call::new(format!("{} #{number} of {}", if lock { "lock" } else { "unlock" }, repo.full))
            .json(json!({ "discussion_locked": lock })),
    )
    .await?;

    let body = expect_body(&answer.body, "web_url", "an issue")?;
    let locked = truthy(at(body, "discussion_locked"));
    Ok(Outcome::ok(
        if locked == lock {
            format!("{} #{number} in {}.", if lock { "Locked" } else { "Unlocked" }, repo.full)
        } else {
            format!("GitLab answered, but #{number} in {} is still {}.", repo.full, if locked { "locked" } else { "unlocked" })
        },
        Fields::new().set("number", number).set("locked", locked),
    ))
}
