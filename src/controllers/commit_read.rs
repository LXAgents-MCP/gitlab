//! Read one commit.
use super::prelude::*;
use crate::utils::diffs::{summarize_file, unified_diff};
use crate::utils::text::head;

pub fn def() -> ToolDef {
    ToolDef {
        name: "commit_read",
        description: "Read one commit of a GitLab project: its message, author, date, parents, the files it changed \
                      with their line counts, and optionally its diff (cut at 20,000 characters). Use it to see what a \
                      commit did. Defaults to the tip of the default branch.",
        input_schema: input(vec![
            repo_arg(),
            opt("ref", string_arg("Commit sha, branch or tag. Default: the default branch's tip.")),
            opt("diff", bool_arg("true also returns the commit's diff. Default false.")),
        ]),
        output_schema: output(vec![
            req("sha", string()),
            req("message", string()),
            opt("author", string()),
            opt("date", string()),
            req("parents", strings()),
            req("additions", num()),
            req("deletions", num()),
            req("file_count", num()),
            req("files", array(file_summary())),
            opt("diff", string()),
            req("truncated", boolean()),
            opt("html_url", string()),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let reference = optional_ref(arg(args, "ref"), "ref")?.unwrap_or_else(|| "HEAD".to_string());
    let want_diff = optional_flag(arg(args, "diff"), "diff", false)?;
    let path = format!("{}/repository/commits/{}", repo.path, ref_path(&reference));

    let answer = gitlab(ctx, "GET", &path, Call::new(format!("read the commit '{reference}' of {}", repo.full))).await?;
    let body = expect_body(&answer.body, "id", "a commit")?;

    // The commit carries its totals but not its files, which are the diff's.
    let changed = gitlab(
        ctx,
        "GET",
        &format!("{path}/diff"),
        Call::new(format!("list the files of the commit '{reference}' of {}", repo.full))
            .query("per_page", Some(MAX_FILES_SHOWN)),
    )
    .await?;
    let listed = list(&changed.body);
    let files: Vec<Value> = listed.iter().map(summarize_file).collect();
    let mut truncated = changed.headers.get("x-next-page").is_some_and(|v| !v.is_empty());

    let mut diff = None;
    if want_diff {
        let (shown, cut) = head(&unified_diff(listed), MAX_DIFF_CHARS);
        diff = Some(shown);
        truncated = truncated || cut;
    }

    let title = at(body, "title");
    let subject = if title.is_null() { first_line(at(body, "message")) } else { js_string(title) };
    let author = at(body, "author_name");
    let sum = |key: &str| files.iter().map(|f| f[key].as_i64().unwrap_or(0)).sum::<i64>();
    let additions = match at(body, "stats.additions") {
        Value::Null => Value::from(sum("additions")),
        v => v.clone(),
    };
    let deletions = match at(body, "stats.deletions") {
        Value::Null => Value::from(sum("deletions")),
        v => v.clone(),
    };
    let parents = match at(body, "parent_ids") {
        Value::Array(p) => Value::Array(p.clone()),
        _ => json!([]),
    };
    let by = if truthy(author) { format!(" by {}", js_string(author)) } else { String::new() };
    Ok(Outcome::ok(
        format!(
            "Commit {}{by}: {}",
            short(at(body, "id")),
            if subject.is_empty() { "(no message)".to_string() } else { subject }
        ),
        Fields::new()
            .raw("sha", at(body, "id"))
            .set("message", or(at(body, "message"), &json!("")).clone())
            .truthy("author", author)
            .truthy("date", at(body, "authored_date"))
            .set("parents", parents)
            .set("additions", additions)
            .set("deletions", deletions)
            .set("file_count", files.len())
            .set("files", files)
            .opt("diff", diff)
            .set("truncated", truncated)
            .truthy("html_url", at(body, "web_url")),
    ))
}
