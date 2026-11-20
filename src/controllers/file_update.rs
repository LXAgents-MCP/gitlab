//! Replace a file's content as one commit.
use super::prelude::*;
use crate::services::contents::{encode_content, explain_commit, files_path};

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_update",
        description: "Replace an existing file's whole content in a GitLab project as one commit, on a branch (default: \
                      the default branch). Pass last_commit_id from file_read to fail if the file changed since. Fails if \
                      the file does not exist; use file_create for a new one. Content up to 1 MB.",
        input_schema: input(vec![
            repo_arg(),
            req("path", string_arg("Path of the file to replace.")),
            req("content", string_arg("The file's whole new content, as text.")),
            req("message", string_arg("Commit message.")),
            opt("branch", string_arg("Branch to commit to. Default: the default branch.")),
            opt(
                "last_commit_id",
                string_arg("The file's last_commit_id from file_read. The update fails if the file has changed since."),
            ),
        ]),
        output_schema: output(vec![req("path", string()), req("branch", string())]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let content = required_raw_text(arg(args, "content"), "content", true)?;
    let message = required_text(arg(args, "message"), "message")?;
    let given = optional_ref(arg(args, "branch"), "branch")?;
    let last_commit = optional_text(arg(args, "last_commit_id"), "last_commit_id")?;
    let encoded = encode_content(&content)?;

    let branch = branch_or_default(ctx, &repo, given).await?;
    let mut json = json!({ "branch": branch, "content": encoded, "encoding": "base64", "commit_message": message });
    if let Some(last) = &last_commit {
        json["last_commit_id"] = Value::from(last.as_str());
    }
    let answer = gitlab(
        ctx,
        "PUT",
        &files_path(&repo, &file),
        Call::new(format!("update '{}' in {} on {branch}", file.path, repo.full))
            .json(json)
            .explain(|status, detail| if status == 400 { explain_commit(detail, &repo, &file, &branch) } else { None }),
    )
    .await?;

    let body = expect_body(&answer.body, "file_path", "a file commit")?;
    let on = or(at(body, "branch"), &Value::from(branch.as_str())).clone();
    Ok(Outcome::ok(
        format!("Updated {} in {} on {}.", file.path, repo.full, js_string(&on)),
        Fields::new().raw("path", at(body, "file_path")).set("branch", on),
    ))
}
