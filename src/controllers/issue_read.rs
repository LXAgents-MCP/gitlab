//! Read one issue, and optionally its discussion.
use super::prelude::*;
use crate::models::shapes::{comment_shape, comments_from, issue_fields, issue_shape};

pub fn def() -> ToolDef {
    let mut out = issue_shape();
    out.push(opt("comments", array(comment_shape())));
    ToolDef {
        name: "issue_read",
        description: "Read one issue of a GitLab project: title, state, author, labels, assignees, milestone and body, \
                      and with `comments: true` its first 30 comments, without the system notes GitLab adds when \
                      something changes. The number is the issue's number in the project (#42), not its global id; \
                      merge requests are numbered separately and are read with pr_read.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Issue number in the project, e.g. 42.")),
            opt("comments", bool_arg("true also returns the first 30 comments. Default false.")),
        ]),
        output_schema: output(out),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let with_comments = optional_flag(arg(args, "comments"), "comments", false)?;

    let answer =
        gitlab(ctx, "GET", &format!("{}/issues/{number}", repo.path), Call::new(format!("read issue #{number} of {}", repo.full)))
            .await?;
    let body = expect_body(&answer.body, "web_url", "an issue")?;
    let fields = issue_fields(body);
    let comment_count = fields.get("comment_count").and_then(Value::as_f64).unwrap_or(0.0);

    let mut comments = None;
    if with_comments && comment_count > 0.0 {
        let notes = gitlab(
            ctx,
            "GET",
            &format!("{}/issues/{number}/notes", repo.path),
            Call::new(format!("read the comments of issue #{number} of {}", repo.full))
                .query("activity_filter", Some("only_comments"))
                .query("order_by", Some("created_at"))
                .query("sort", Some("asc"))
                .query("per_page", Some(MAX_COMMENTS_SHOWN)),
        )
        .await?;
        // `only_comments` is GitLab's own filter; `comments_from` drops a system note as well, in case a
        // version of GitLab ignores the filter.
        comments = Some(comments_from(&notes.body));
    }

    let more = match &comments {
        Some(shown) if comment_count > shown.len() as f64 => format!("; showing the first {} comments", shown.len()),
        _ => String::new(),
    };
    let state = fields.get("state").map(js_string).unwrap_or_default();
    let title = fields.get("title").map(js_string).unwrap_or_default();
    Ok(Outcome::ok(
        format!("Issue #{number} in {} is {state}: {title}{more}.", repo.full),
        fields.opt("comments", comments),
    ))
}
