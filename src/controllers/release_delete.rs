//! Delete a release.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_delete",
        description: "Delete the release of a tag in a GitLab project, with its notes and asset links. Destructive, so \
                      confirm with the owner first. The tag is kept; use tag_delete to remove it.",
        input_schema: input(vec![repo_arg(), req("tag", string_arg("Tag of the release to delete."))]),
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
        &format!("{}/releases/{}", repo.path, ref_path(&tag)),
        Call::new(format!("delete the release for '{tag}' of {}", repo.full)).explain(|status, _| {
            (status == 404)
                .then(|| format!("{} has no release for the tag '{tag}'. List what exists with release_list.", repo.full))
        }),
    )
    .await?;

    Ok(Outcome::ok(
        format!("Deleted the release for {tag} of {}. Its tag was kept.", repo.full),
        Fields::new().set("deleted", true).set("tag", tag.clone()),
    ))
}
