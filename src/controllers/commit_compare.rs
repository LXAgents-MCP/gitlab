//! Compare two refs.
use super::prelude::*;
use crate::utils::diffs::summarize_file;

pub fn def() -> ToolDef {
    ToolDef {
        name: "commit_compare",
        description: "Compare two branches, tags or commits of a GitLab project: how far ahead or behind the head \
                      is of the base, the most recent 50 commits between them, and the changed files with line \
                      counts. Use it to see what a branch would bring in before merging.",
        input_schema: input(vec![
            repo_arg(),
            req("base", string_arg("The ref to compare from, e.g. 'main'.")),
            req("head", string_arg("The ref to compare, e.g. 'feat/login'.")),
        ]),
        output_schema: output(vec![
            req("status", string()),
            req("ahead_by", num()),
            req("behind_by", num()),
            req("total_commits", num()),
            req("commits", array(obj(vec![req("sha", string()), req("message", string()), opt("author", string())]))),
            req("file_count", num()),
            req("files", array(file_summary())),
            req("truncated", boolean()),
            opt("html_url", string()),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let base = ref_name(arg(args, "base"), "base")?;
    let head = ref_name(arg(args, "head"), "head")?;
    let full = &repo.full;
    let explain = |status: u16, detail: &str| {
        matches!(status, 404 | 400).then(|| {
            format!(
                "GitLab could not find '{base}' or '{head}' in {full} ({status}){detail}. Check both names, and that the token can see the project."
            )
        })
    };
    let path = format!("{}/repository/compare", repo.path);

    let body = gitlab(
        ctx,
        "GET",
        &path,
        Call::new(format!("compare '{head}' with '{base}' in {full}"))
            .query("from", Some(&base))
            .query("to", Some(&head))
            .explain(explain),
    )
    .await?
    .body;
    let Value::Array(all) = at(&body, "commits") else {
        return Err(Refused::new("GitLab answered, but the body was not a comparison this server recognizes."));
    };

    // GitLab compares in one direction, so how far behind is the same question asked the other way round.
    let back = gitlab(
        ctx,
        "GET",
        &path,
        Call::new(format!("compare '{base}' with '{head}' in {full}"))
            .query("from", Some(&head))
            .query("to", Some(&base))
            .explain(explain),
    )
    .await?
    .body;

    let ahead = all.len();
    let behind = match at(&back, "commits") {
        Value::Array(commits) => commits.len(),
        _ => 0,
    };
    let listed = list(at(&body, "diffs"));
    // Oldest first, whatever order GitLab sent them in, and the newest are the ones a reader came for. A
    // list with a date that does not parse keeps GitLab's order.
    let mut by_date: Vec<&Value> = all.iter().collect();
    let dates: Vec<Option<i64>> =
        by_date.iter().map(|c| at(c, "created_at").as_str().and_then(date_millis)).collect();
    if dates.iter().all(Option::is_some) {
        let mut paired: Vec<(i64, &Value)> = dates.into_iter().flatten().zip(by_date).collect();
        paired.sort_by_key(|(date, _)| *date);
        by_date = paired.into_iter().map(|(_, commit)| commit).collect();
    }
    let start = by_date.len().saturating_sub(MAX_COMMITS_SHOWN);
    let commits: Vec<Value> = by_date[start..]
        .iter()
        .map(|commit| {
            Fields::new()
                .raw("sha", at(commit, "id"))
                .set("message", or(at(commit, "title"), &json!("")).clone())
                .truthy("author", at(commit, "author_name"))
                .into_value()
        })
        .collect();
    let files: Vec<Value> = listed.iter().take(MAX_FILES_SHOWN).map(summarize_file).collect();

    let status = match (ahead, behind) {
        (0, 0) => "identical",
        (_, 0) => "ahead",
        (0, _) => "behind",
        _ => "diverged",
    };
    let sentence = match status {
        "identical" => format!("{head} and {base} are identical."),
        "ahead" => format!("{head} is {ahead} {} ahead of {base}.", plural(ahead, "commit", "commits")),
        "behind" => format!("{head} is {behind} {} behind {base}.", plural(behind, "commit", "commits")),
        _ => format!("{head} has diverged from {base}: {ahead} ahead, {behind} behind."),
    };

    Ok(Outcome::ok(
        sentence,
        Fields::new()
            .set("status", status)
            .set("ahead_by", ahead)
            .set("behind_by", behind)
            .set("total_commits", ahead)
            .set("commits", commits)
            .set("file_count", listed.len())
            .set("files", files)
            .set(
                "truncated",
                ahead > MAX_COMMITS_SHOWN || listed.len() > MAX_FILES_SHOWN || at(&body, "compare_timeout") == &json!(true),
            )
            .truthy("html_url", at(&body, "web_url")),
    ))
}
