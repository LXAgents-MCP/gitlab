//! Delete a tag.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "tag_delete",
        description: "Delete a tag from a GitLab project. Destructive: anything that names the tag, such as a build or a \
                      download link, stops working, so confirm with the owner first. It does not delete a release made \
                      from the tag. A protected tag cannot be deleted.",
        input_schema: input(vec![repo_arg(), req("tag", string_arg("Name of the tag to delete."))]),
        output_schema: output(vec![req("deleted", boolean()), req("tag", string())]),
        annotations: DESTROY,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let tag = ref_name(arg(args, "tag"), "tag")?;

    gitlab(
        ctx,
        "DELETE",
        &format!("{}/repository/tags/{}", repo.path, ref_path(&tag)),
        Call::new(format!("delete the tag '{tag}' from {}", repo.full)).explain(|status, detail| {
            (status == 403 || status == 404).then(|| {
                format!(
                    "GitLab would not delete the tag '{tag}' from {} ({status}){detail}. It may not exist \
                     (list the tags with tag_get_all), or it is protected, or the token's user may not delete tags.",
                    repo.full
                )
            })
        }),
    )
    .await?;

    Ok(Outcome::ok(
        format!("Deleted tag {tag} from {}.", repo.full),
        Fields::new().set("deleted", true).set("tag", tag.clone()),
    ))
}
