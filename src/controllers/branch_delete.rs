//! Delete a branch.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_delete",
        description: "Delete a branch from a GitLab project. Destructive: commits only that branch holds become \
                      unreachable, so confirm with the owner first. The default branch and protected branches cannot \
                      be deleted.",
        input_schema: input(vec![repo_arg(), req("branch", string_arg("Name of the branch to delete."))]),
        output_schema: output(vec![req("deleted", boolean()), req("branch", string())]),
        annotations: DESTROY,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let branch = ref_name(arg(args, "branch"), "branch")?;

    gitlab(
        ctx,
        "DELETE",
        &format!("{}/repository/branches/{}", repo.path, ref_path(&branch)),
        Call::new(format!("delete the branch '{branch}' from {}", repo.full)).explain(|status, detail| {
            matches!(status, 400 | 403 | 404 | 422).then(|| {
                format!(
                    "GitLab would not delete the branch '{branch}' from {} ({status}){detail}. The branch may not exist, \
                     or it is the default branch or a protected branch, which cannot be deleted; deleting needs the \
                     Developer role or more.",
                    repo.full
                )
            })
        }),
    )
    .await?;

    Ok(Outcome::ok(
        format!("Deleted branch {branch} from {}.", repo.full),
        Fields::new().set("deleted", true).set("branch", branch),
    ))
}
