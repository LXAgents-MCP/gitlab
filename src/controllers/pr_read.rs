//! Read one merge request.
use super::prelude::*;
use crate::models::shapes::{comment_shape, comments_from, names_in};
use crate::services::merge_status::mergeability;
use crate::utils::diffs::summarize_file;

pub fn def() -> ToolDef {
    ToolDef {
        name: "pr_read",
        description: "Read one merge request of a GitLab project: state, draft, whether it can be merged and if not why, \
                      branches, labels, reviewers and description; with `files: true` the first 100 changed files with line \
                      counts, and with `comments: true` its first 30 comments. The number is the merge request's number in \
                      the project (!7), which is separate from issue numbers.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Merge request number in the project, e.g. 7 for !7.")),
            opt("files", bool_arg("true also lists the changed files, with line counts. Default false.")),
            opt("comments", bool_arg("true also returns the first 30 comments, without system notes. Default false.")),
        ]),
        output_schema: output(vec![
            req("number", num()),
            req("title", string()),
            req("state", string()),
            req("draft", boolean()),
            req("merged", boolean()),
            opt("mergeable", boolean()),
            opt("merge_status", string()),
            opt("author", string()),
            req("head", string()),
            req("base", string()),
            req("head_sha", string()),
            opt("changed_files", string()),
            req("labels", strings()),
            req("assignees", strings()),
            req("reviewers", strings()),
            req("comment_count", num()),
            req("html_url", string()),
            opt("created_at", string()),
            opt("updated_at", string()),
            opt("merged_at", string()),
            opt("body", string()),
            opt("files", array(file_summary())),
            opt("comments", array(comment_shape())),
        ]),
        annotations: READ,
    }
}

/// JavaScript's `value ?? fallback`.
fn or_else(value: &Value, fallback: Value) -> Value {
    if value.is_null() { fallback } else { value.clone() }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let with_files = optional_flag(arg(args, "files"), "files", false)?;
    let with_comments = optional_flag(arg(args, "comments"), "comments", false)?;
    let path = format!("{}/merge_requests/{number}", repo.path);

    let answer = gitlab(ctx, "GET", &path, Call::new(format!("read merge request !{number} of {}", repo.full))).await?;
    let body = expect_body(&answer.body, "web_url", "a merge request")?;
    let comment_count = or_else(at(body, "user_notes_count"), json!(0));
    let count = comment_count.as_f64().unwrap_or(0.0);

    let mut files = None;
    if with_files {
        let listed = gitlab(
            ctx,
            "GET",
            &format!("{path}/diffs"),
            Call::new(format!("list the files of merge request !{number} of {}", repo.full))
                .query("per_page", Some(MAX_FILES_SHOWN)),
        )
        .await?;
        files = Some(list(&listed.body).iter().map(summarize_file).collect::<Vec<_>>());
    }

    let mut comments = None;
    if with_comments && count > 0.0 {
        let notes = gitlab(
            ctx,
            "GET",
            &format!("{path}/notes"),
            Call::new(format!("read the comments of merge request !{number} of {}", repo.full))
                .query("activity_filter", Some("only_comments"))
                .query("order_by", Some("created_at"))
                .query("sort", Some("asc"))
                .query("per_page", Some(MAX_COMMENTS_SHOWN)),
        )
        .await?;
        comments = Some(comments_from(&notes.body));
    }

    let state = at(body, "state");
    let merged = state.as_str() == Some("merged");
    let open = state.as_str() == Some("opened");
    let draft = truthy(&or_else(at(body, "draft"), at(body, "work_in_progress").clone()));
    let verdict = mergeability(body);
    // While GitLab is still working it out, "can it be merged" has no answer yet, which is not "no".
    let known = open && !verdict.pending;
    let changed = match at(body, "changes_count") {
        Value::Null => None,
        value => Some(js_string(value)),
    };
    let head = or_else(at(body, "source_branch"), json!(""));
    let base = or_else(at(body, "target_branch"), json!(""));

    let mut summary = format!(
        "Merge request !{number} in {} is {}: {} ({} into {}).",
        repo.full,
        if draft && open { "a draft, opened".to_string() } else { js_string(state) },
        js_string(at(body, "title")),
        js_string(&head),
        js_string(&base)
    );
    if open && verdict.pending {
        summary.push_str(" Whether it can be merged is still being worked out; ask again in a moment.");
    }
    if known && !verdict.ready {
        summary.push_str(&format!(" It cannot be merged yet: {}.", verdict.reason));
    }
    if let Some(shown) = &comments
        && count > shown.len() as f64 {
            summary.push_str(&format!(" Showing the first {} comments.", shown.len()));
        }

    let author = at(body, "author.username");
    let mut fields = Fields::new()
        .set("number", number)
        .raw("title", at(body, "title"))
        .raw("state", state)
        .set("draft", draft)
        .set("merged", merged);
    if known {
        fields = fields.set("mergeable", verdict.ready).set("merge_status", verdict.status.clone());
    }
    let fields = fields
        .truthy("author", author)
        .set("head", head)
        .set("base", base)
        .set("head_sha", or_else(at(body, "sha"), json!("")))
        .opt("changed_files", changed)
        .set("labels", names_in(at(body, "labels"), "name"))
        .set("assignees", names_in(at(body, "assignees"), "username"))
        .set("reviewers", names_in(at(body, "reviewers"), "username"))
        .set("comment_count", comment_count)
        .raw("html_url", at(body, "web_url"))
        .truthy("created_at", at(body, "created_at"))
        .truthy("updated_at", at(body, "updated_at"))
        .truthy("merged_at", at(body, "merged_at"))
        .truthy("body", at(body, "description"))
        .opt("files", files)
        .opt("comments", comments);
    Ok(Outcome::ok(summary, fields))
}
