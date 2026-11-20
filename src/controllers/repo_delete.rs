//! Delete a project.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_delete",
        description: "Delete a GitLab project, with its issues, merge requests and wiki. Destructive: confirm with the \
                      owner first. GitLab may only mark it for deletion and remove it after a retention period, during \
                      which it can be restored. `confirm` must repeat the project's full path. Needs the Owner role.",
        input_schema: input(vec![
            repo_arg(),
            req("confirm", string_arg("The project's full path again, 'group/project'. Anything else deletes nothing.")),
        ]),
        output_schema: output(vec![req("accepted", boolean()), req("full_name", string())]),
        annotations: DESTROY,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let confirm = required_text(arg(args, "confirm"), "confirm")?;

    // A numeric id says nothing a person could type back, so it is resolved to the path first.
    let mut full = repo.full.clone();
    if !full.is_empty() && full.bytes().all(|b| b.is_ascii_digit()) {
        let body = gitlab(ctx, "GET", &repo.path, Call::new(format!("read the project {}", repo.full))).await?.body;
        full = js_string(at(expect_body(&body, "path_with_namespace", "a project")?, "path_with_namespace"));
    }

    // The guard is mechanical on purpose: a path typed back cannot be a reflex, a copied flag can.
    if confirm.to_lowercase() != full.to_lowercase() {
        return Err(Refused(format!(
            "`confirm` must repeat the project's full path, '{full}', to delete it. Nothing was deleted."
        )));
    }

    gitlab(
        ctx,
        "DELETE",
        &repo.path,
        Call::new(format!("delete the project {full}")).explain(|status, detail| {
            (status == 403).then(|| {
                format!(
                    "GitLab refused to delete {full} (403){detail}. Deleting needs the Owner role on the project, \
                     or the right to delete in its group, and a token with the `api` scope."
                )
            })
        }),
    )
    .await?;

    Ok(Outcome::ok(
        format!(
            "GitLab accepted the deletion of {full}. It may mark the project for deletion and remove it after a \
             retention period, so it can still be restored until then; read it again to see whether it is scheduled."
        ),
        Fields::new().set("accepted", true).set("full_name", full.clone()),
    ))
}
