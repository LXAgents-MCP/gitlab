//! Merge a merge request.
use super::prelude::*;
use crate::services::failure::api_detail;
use crate::services::merge_status::mergeability;

// The statuses GitLab answers with when the request cannot be merged as it stands.
const CANNOT_MERGE: &[u16] = &[405, 406, 422];

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_merge",
        description: "Merge a merge request in a GitLab project. How it lands (a merge commit, a fast-forward or a rebase) is \
                      the project's setting and cannot be chosen here; `squash` joins its commits into one first. The source \
                      branch is removed only if the merge request was set to. Destructive: it moves the target branch, so confirm \
                      with the owner first. Pass `sha` to merge only if the branch is still at the commit that was reviewed.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Merge request number in the project.")),
            opt("squash", bool_arg("true squashes the commits into one. Default: the merge request's own setting.")),
            opt(
                "message",
                string_arg("Message for the squash commit when squashing, otherwise for the merge commit. Default: GitLab's."),
            ),
            opt("sha", string_arg("The head commit the merge must match; a moved branch is refused (409).")),
        ]),
        output_schema: output(vec![req("merged", boolean()), opt("state", string()), opt("sha", string())]),
        annotations: DESTROY,
    }
}

/// A JavaScript `??`: the value unless it is null or missing.
fn present(value: &Value) -> Option<&Value> {
    (!value.is_null()).then_some(value)
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let squash = maybe_flag(arg(args, "squash"), "squash")?;
    let message = raw_text(arg(args, "message"), "message")?;
    let sha = optional_text(arg(args, "sha"), "sha")?;
    let path = format!("{}/merge_requests/{number}", repo.path);
    let what = format!("merge merge request !{number} of {}", repo.full);

    let mut body = serde_json::Map::new();
    if let Some(squash) = squash {
        body.insert("squash".into(), squash.into());
    }
    if let Some(message) = message {
        let key = if squash == Some(true) { "squash_commit_message" } else { "merge_commit_message" };
        body.insert(key.into(), message.into());
    }
    if let Some(sha) = sha {
        body.insert("sha".into(), sha.into());
    }

    let answer = gitlab(
        ctx,
        "PUT",
        &format!("{path}/merge"),
        Call::new(what.clone()).json(Value::Object(body)).allow(CANNOT_MERGE).explain(|status, detail| match status {
            401 => Some(format!(
                "GitLab would not let this token merge !{number} in {} (401){detail}. Either the token is not valid, \
                 or its user may not merge into the target branch, which for a protected branch takes the Maintainer role \
                 or a role the branch's protection allows.",
                repo.full
            )),
            409 => Some(format!(
                "GitLab did not merge !{number} in {} (409){detail}. The branch moved since the commit that was \
                 meant to be merged; read the merge request again with pr_read.",
                repo.full
            )),
            _ => None,
        }),
    )
    .await?;

    if CANNOT_MERGE.contains(&answer.status) {
        // GitLab says only that it cannot, so ask it why: it knows whether the cause is a draft, a pipeline, an approval or a conflict.
        let mut why = String::new();
        // The reason is a courtesy; the refusal stands without it.
        if let Ok(read) = gitlab(ctx, "GET", &path, Call::new(format!("read merge request !{number} of {}", repo.full))).await {
            let current = read.body;
            let state = at(&current, "state");
            let verdict = mergeability(&current);
            why = if state.as_str() != Some("opened") {
                format!(" It is {}.", present(state).map(js_string).unwrap_or_else(|| "not open".into()))
            } else if verdict.ready {
                " GitLab reports it as mergeable, so the project's merge settings refused it.".into()
            } else if verdict.pending {
                " GitLab is still working out whether it can be merged; try again in a moment.".into()
            } else {
                format!(" It cannot be merged yet: {}.", verdict.reason)
            };
        }
        return Err(Refused(format!("GitLab would not {what} ({}){}.{why}", answer.status, api_detail(&answer.body))));
    }

    let body = answer.body;
    let state = at(&body, "state").as_str().map(str::to_string);
    let merged = state.as_deref() == Some("merged");
    let commit = present(at(&body, "merge_commit_sha")).or_else(|| present(at(&body, "squash_commit_sha"))).filter(|c| truthy(c));
    let summary = if merged {
        let at_commit = commit.map(|c| format!(", at {}", short(c))).unwrap_or_default();
        format!("Merged merge request !{number} in {}{at_commit}.", repo.full)
    } else if state.as_deref() == Some("locked") {
        format!("GitLab accepted the merge of !{number} in {} and is completing it; read it with pr_read.", repo.full)
    } else {
        format!(
            "GitLab answered the merge of !{number} in {} without saying it merged (it is {}); read it with pr_read.",
            repo.full,
            state.as_deref().unwrap_or("in an unknown state")
        )
    };
    let ok = merged || state.as_deref() == Some("locked");
    Ok(Outcome::with_ok(
        summary,
        Fields::new()
            .set("merged", merged)
            .opt("state", state.filter(|s| !s.is_empty()))
            .opt("sha", commit.cloned()),
        ok,
    ))
}
