//! Approve a merge request.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_approve",
        description: "Approve a merge request in a GitLab project as the token's user. GitLab approvals take no message. \
                      Whether a person may approve their own merge request, and whether an approval is needed to merge at all, \
                      are the project's settings. Approving again changes nothing more.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Merge request number in the project.")),
            opt(
                "sha",
                string_arg("The head commit being approved; the approval fails if the branch has moved on. Default: the latest."),
            ),
        ]),
        output_schema: output(vec![req("number", num()), req("approved_by", strings()), opt("approvals_left", num())]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let sha = optional_text(arg(args, "sha"), "sha")?;

    let body = gitlab(
        ctx,
        "POST",
        &format!("{}/merge_requests/{number}/approve", repo.path),
        Call::new(format!("approve merge request !{number} of {}", repo.full))
            .json_opt(sha.map(|sha| json!({ "sha": sha })))
            .explain(|status, detail| match status {
                401 => Some(format!(
                    "GitLab would not record the approval on !{number} of {} (401){detail}. The token's user may not \
                     approve this merge request: the project may forbid approving your own, or the user lacks the role, \
                     or the token is not valid.",
                    repo.full
                )),
                409 => Some(format!(
                    "GitLab did not record the approval on !{number} of {} (409){detail}. The branch moved on from \
                     the commit that was meant to be approved; read the merge request again with pr_read.",
                    repo.full
                )),
                _ => None,
            }),
    )
    .await?
    .body;

    let Some(entries) = at(&body, "approved_by").as_array() else {
        return Err(Refused::new("GitLab answered, but the body was not an approval this server recognizes."));
    };
    let approved_by: Vec<String> =
        entries.iter().filter_map(|entry| at(entry, "user.username").as_str().map(str::to_string)).collect();
    let left = as_int(at(&body, "approvals_left"));
    let who = if approved_by.is_empty() { "nobody listed".to_string() } else { approved_by.join(", ") };
    let tail = match left {
        None => ".".to_string(),
        Some(0) => ", no approvals left to wait for.".to_string(),
        Some(n) => format!(", {n} more needed."),
    };
    Ok(Outcome::ok(
        format!("Approved merge request !{number} in {}; approved by {who}{tail}", repo.full),
        Fields::new().set("number", number).set("approved_by", approved_by).opt("approvals_left", left),
    ))
}
