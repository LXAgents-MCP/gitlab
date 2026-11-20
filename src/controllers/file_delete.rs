//! Delete a file as one commit.
use super::prelude::*;
use crate::services::contents::{explain_commit, files_path};

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_delete",
        description: "Delete one file from a GitLab project as one commit, on a branch (default: the default branch). \
                      Destructive: the file stays in history, but confirm with the owner first. Pass last_commit_id from \
                      file_read to fail if the file changed since.",
        input_schema: input(vec![
            repo_arg(),
            req("path", string_arg("Path of the file to delete.")),
            req("message", string_arg("Commit message.")),
            opt("branch", string_arg("Branch to commit to. Default: the default branch.")),
            opt(
                "last_commit_id",
                string_arg("The file's last_commit_id from file_read. The delete fails if the file has changed since."),
            ),
        ]),
        output_schema: output(vec![req("deleted", boolean()), req("path", string()), req("branch", string())]),
        annotations: DESTROY,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let message = required_text(arg(args, "message"), "message")?;
    let given = optional_ref(arg(args, "branch"), "branch")?;
    let last_commit = optional_text(arg(args, "last_commit_id"), "last_commit_id")?;

    let branch = branch_or_default(ctx, &repo, given).await?;
    let mut json = json!({ "branch": branch, "commit_message": message });
    if let Some(last) = &last_commit {
        json["last_commit_id"] = Value::from(last.as_str());
    }
    gitlab(
        ctx,
        "DELETE",
        &files_path(&repo, &file),
        Call::new(format!("delete '{}' from {} on {branch}", file.path, repo.full))
            .json(json)
            .explain(|status, detail| if status == 400 { explain_commit(detail, &repo, &file, &branch) } else { None }),
    )
    .await?;

    Ok(Outcome::ok(
        format!("Deleted {} from {} on {branch}.", file.path, repo.full),
        Fields::new().set("deleted", true).set("path", file.path.clone()).set("branch", branch.clone()),
    ))
}
