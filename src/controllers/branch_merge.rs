//! Merge one branch into another, through a merge request that is accepted at once.
use std::time::Duration;

use super::prelude::*;
use crate::services::merge_status::mergeability;

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_merge",
        description: "Merge one branch into another in a GitLab project. GitLab has no direct merge, so this opens a merge \
                      request from head into base and accepts it at once; the merged request stays as the record, and if \
                      it cannot be merged the request is closed again. Destructive: it moves the base branch, so confirm \
                      with the owner first. Use pr_merge when a merge request exists.",
        input_schema: input(vec![
            repo_arg(),
            req("base", string_arg("Branch that receives the merge, e.g. 'main'.")),
            req("head", string_arg("Branch to merge into the base.")),
            opt("message", string_arg("Message for the merge commit. Default: GitLab's.")),
        ]),
        output_schema: output(vec![
            req("merged", boolean()),
            req("up_to_date", boolean()),
            opt("sha", string()),
            opt("merge_request", num()),
            opt("html_url", string()),
        ]),
        annotations: DESTROY,
    }
}

/// Close a merge request this tool opened and could not merge, and say so in words for a refusal.
async fn close_again(ctx: &Ctx, repo: &Repo, iid: i64) -> String {
    let closed = gitlab(
        ctx,
        "PUT",
        &format!("{}/merge_requests/{iid}", repo.path),
        Call::new(format!("close the merge request !{iid} of {}", repo.full)).json(json!({ "state_event": "close" })),
    )
    .await;
    match closed {
        Ok(_) => format!("The merge request it opened, !{iid}, was closed again."),
        Err(_) => format!("The merge request it opened, !{iid}, is still open; close it with pr_close."),
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let base = ref_name(arg(args, "base"), "base")?;
    let head = ref_name(arg(args, "head"), "head")?;
    let message = optional_text(arg(args, "message"), "message")?;
    let full = &repo.full;

    // Nothing to merge is an answer, and it must not leave a merge request behind.
    let compared = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/compare", repo.path),
        Call::new(format!("compare '{head}' with '{base}' in {full}"))
            .query("from", Some(&base))
            .query("to", Some(&head))
            .explain(|status, detail| {
                matches!(status, 404 | 400).then(|| {
                    format!(
                        "GitLab could not find '{base}' or '{head}' in {full} ({status}){detail}. Check both names, \
                         and that the token can see the project."
                    )
                })
            }),
    )
    .await?;
    if let Value::Array(commits) = at(&compared.body, "commits")
        && commits.is_empty()
    {
        return Ok(Outcome::ok(
            format!("Nothing to merge: {base} already contains {head}."),
            Fields::new().set("merged", false).set("up_to_date", true),
        ));
    }

    let opened = gitlab(
        ctx,
        "POST",
        &format!("{}/merge_requests", repo.path),
        Call::new(format!("open a merge request from '{head}' into '{base}' in {full}"))
            .json(json!({
                "source_branch": head,
                "target_branch": base,
                "title": format!("Merge {head} into {base}"),
                "remove_source_branch": false,
            }))
            .explain(|status, detail| {
                (status == 409).then(|| {
                    format!(
                        "GitLab could not open a merge request from '{head}' into '{base}' in {full} (409){detail}. \
                         One is already open; merge that one with pr_merge."
                    )
                })
            }),
    )
    .await?
    .body;
    let Some(iid) = as_int(at(&opened, "iid")) else {
        return Err(Refused::new("GitLab answered, but the body was not a merge request this server recognizes."));
    };

    // GitLab works out whether it can merge in the background; ask again until it has, or give up.
    let mut current = opened.clone();
    let mut tries = 0;
    while mergeability(&current).pending && tries < MERGE_POLL_TRIES {
        ctx.sleep.sleep(Duration::from_millis(MERGE_POLL_MS)).await;
        current = gitlab(
            ctx,
            "GET",
            &format!("{}/merge_requests/{iid}", repo.path),
            Call::new(format!("read the merge request !{iid} of {full}")),
        )
        .await?
        .body;
        tries += 1;
    }

    let verdict = mergeability(&current);
    if !verdict.ready {
        let closed = close_again(ctx, &repo, iid).await;
        return Err(Refused(if verdict.pending {
            format!("GitLab is still working out whether '{head}' can be merged into '{base}' in {full}. {closed} Try again in a moment.")
        } else {
            format!("GitLab will not merge '{head}' into '{base}' in {full}: {}. {closed}", verdict.reason)
        }));
    }

    let mut json = json!({
        "sha": or(at(&current, "sha"), at(&opened, "sha")).clone(),
        "should_remove_source_branch": false,
    });
    if at(&current, "sha").is_null() && at(&opened, "sha").is_null() {
        json.as_object_mut().expect("object").remove("sha");
    }
    if let Some(message) = &message {
        json["merge_commit_message"] = Value::from(message.as_str());
    }
    let merged = gitlab(
        ctx,
        "PUT",
        &format!("{}/merge_requests/{iid}/merge", repo.path),
        Call::new(format!("merge '{head}' into '{base}' in {full}")).json(json).explain(|status, detail| match status {
            401 => Some(format!(
                "GitLab would not let this token merge into '{base}' in {full} (401){detail}. Merging into a protected \
                 branch needs the Maintainer role, or a role the branch's protection allows."
            )),
            405 | 422 => Some(format!(
                "GitLab could not merge '{head}' into '{base}' in {full} ({status}){detail}. It cannot be merged as it stands."
            )),
            409 => Some(format!(
                "GitLab did not merge '{head}' into '{base}' in {full} (409){detail}. '{head}' moved while the merge was being made; try again."
            )),
            _ => None,
        }),
    )
    .await;
    let merged = match merged {
        Ok(answer) => answer.body,
        Err(refused) => return Err(Refused(format!("{} {}", refused.0, close_again(ctx, &repo, iid).await))),
    };

    let sha = or(at(&merged, "merge_commit_sha"), at(&merged, "squash_commit_sha")).clone();
    let state = at(&merged, "state");
    let done = state.as_str() == Some("merged");
    let locked = state.as_str() == Some("locked");
    let summary = if done {
        let at_sha = if truthy(&sha) { format!(", at {}", short(&sha)) } else { String::new() };
        format!("Merged {head} into {base} in {full}{at_sha}, through merge request !{iid}.")
    } else if locked {
        format!("GitLab accepted the merge of {head} into {base} in {full} (merge request !{iid}) and is completing it; read it with pr_read.")
    } else {
        let state = if state.is_null() { "in an unknown state".to_string() } else { js_string(state) };
        format!(
            "GitLab answered the merge of {head} into {base} in {full} without saying it merged (merge request !{iid} is {state}); read it with pr_read."
        )
    };
    Ok(Outcome::with_ok(
        summary,
        Fields::new()
            .set("merged", done)
            .set("up_to_date", false)
            .truthy("sha", &sha)
            .set("merge_request", iid)
            .opt("html_url", at(&opened, "web_url").as_str()),
        done || locked,
    ))
}
