//! Ask people to review a merge request.
use super::prelude::*;
use crate::models::shapes::names_in;
use crate::services::users::{bare_username, find_users};

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_review_request",
        description: "Request a review of a merge request in a GitLab project from people, who are added to those already \
                      asked. An unknown username refuses the call before anything changes. GitLab has no team reviewers. \
                      The result lists any person GitLab did not add.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Merge request number in the project.")),
            req("reviewers", string_list_arg("GitLab usernames to ask. At least one, at most 10.")),
        ]),
        output_schema: output(vec![req("number", num()), req("reviewers", strings()), opt("not_applied", strings())]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let requested: Vec<String> = text_list(arg(args, "reviewers"), "reviewers", ListOptions { required: true, max: 10 })?
        .unwrap_or_default()
        .iter()
        .map(|name| bare_username(name))
        .collect();
    let path = format!("{}/merge_requests/{number}", repo.path);
    let what = format!("request a review of merge request !{number} of {}", repo.full);

    // GitLab replaces the whole list of reviewers, so the ones already asked are read and sent back with the new.
    let before =
        gitlab(ctx, "GET", &path, Call::new(format!("read the reviewers of merge request !{number} of {}", repo.full)))
            .await?
            .body;
    expect_body(&before, "web_url", "a merge request")?;
    let asked: Vec<i64> = list(at(&before, "reviewers")).iter().filter_map(|user| as_int(at(user, "id"))).collect();

    let people = find_users(ctx, &requested, &what).await?;
    let mut next: Vec<i64> = Vec::new();
    for id in asked.iter().copied().chain(people.iter().map(|person| person.id)) {
        if !next.contains(&id) {
            next.push(id);
        }
    }

    let mut current = before;
    if next.len() != asked.len() {
        current = gitlab(ctx, "PUT", &path, Call::new(what.clone()).json(json!({ "reviewer_ids": next }))).await?.body;
        expect_body(&current, "web_url", "a merge request")?;
    }

    let now = names_in(at(&current, "reviewers"), "username");
    let not_applied: Vec<String> = requested
        .iter()
        .filter(|name| !now.iter().any(|entry| entry.to_lowercase() == name.to_lowercase()))
        .cloned()
        .collect();
    let who = if now.is_empty() { "nobody".to_string() } else { now.join(", ") };
    let note = if not_applied.is_empty() {
        String::new()
    } else {
        format!(" GitLab did not add: {}. They may not be able to review this project.", not_applied.join(", "))
    };
    Ok(Outcome::ok(
        format!("Review requested on !{number} in {} from: {who}.{note}", repo.full),
        Fields::new()
            .set("number", number)
            .set("reviewers", now)
            .opt("not_applied", (!not_applied.is_empty()).then_some(not_applied)),
    ))
}
