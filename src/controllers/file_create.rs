//! Create a file as one commit.
use super::prelude::*;
use crate::services::contents::{encode_content, explain_commit, files_path};

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_create",
        description: "Create a new file in a GitLab project as one commit, on a branch (default: the default branch). \
                      Fails if the file already exists; use file_update to change it. Content up to 1 MB.",
        input_schema: input(vec![
            repo_arg(),
            req("path", string_arg("Path of the new file, e.g. 'docs/guide.md'.")),
            req("content", string_arg("The file's whole content, as text.")),
            req("message", string_arg("Commit message.")),
            opt("branch", string_arg("Branch to commit to. Default: the default branch.")),
        ]),
        output_schema: output(vec![req("path", string()), req("branch", string())]),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let content = required_raw_text(arg(args, "content"), "content", true)?;
    let message = required_text(arg(args, "message"), "message")?;
    let given = optional_ref(arg(args, "branch"), "branch")?;
    let encoded = encode_content(&content)?;

    let branch = branch_or_default(ctx, &repo, given).await?;
    let answer = gitlab(
        ctx,
        "POST",
        &files_path(&repo, &file),
        Call::new(format!("create '{}' in {} on {branch}", file.path, repo.full))
            .json(json!({ "branch": branch, "content": encoded, "encoding": "base64", "commit_message": message }))
            .explain(|status, detail| if status == 400 { explain_commit(detail, &repo, &file, &branch) } else { None }),
    )
    .await?;

    let body = expect_body(&answer.body, "file_path", "a file commit")?;
    let on = or(at(body, "branch"), &Value::from(branch.as_str())).clone();
    Ok(Outcome::ok(
        format!("Created {} in {} on {}.", file.path, repo.full, js_string(&on)),
        Fields::new().raw("path", at(body, "file_path")).set("branch", on),
    ))
}
