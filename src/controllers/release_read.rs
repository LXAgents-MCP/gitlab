//! Read one release.
use super::prelude::*;
use crate::models::shapes::{describe_release, release_fields, release_shape};

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_read",
        description: "Read one release of a GitLab project: title, notes, release date, and its asset links and source \
                      archives. Give a tag, or omit it for the latest release.",
        input_schema: input(vec![repo_arg(), opt("tag", string_arg("Tag of the release. Omit for the latest release."))]),
        output_schema: output(release_shape()),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let tag = match arg(args, "tag") {
        Value::Null => None,
        tag => Some(ref_name(tag, "tag")?),
    };

    // The latest release is a permanent address that redirects to the release itself.
    let release = match &tag {
        None => {
            gitlab(
                ctx,
                "GET",
                &format!("{}/releases/permalink/latest", repo.path),
                Call::new(format!("read the latest release of {}", repo.full)).explain(|status, _| {
                    (status == 404).then(|| format!("{} has no release yet. List what exists with release_list.", repo.full))
                }),
            )
            .await?
        }
        Some(tag) => {
            gitlab(
                ctx,
                "GET",
                &format!("{}/releases/{}", repo.path, ref_path(tag)),
                Call::new(format!("read the release for '{tag}' of {}", repo.full)).explain(|status, _| {
                    (status == 404).then(|| {
                        format!("{} has no release for the tag '{tag}'. List what exists with release_list.", repo.full)
                    })
                }),
            )
            .await?
        }
    }
    .body;

    expect_body(&release, "tag_name", "a release")?;
    let fields = release_fields(&release);
    Ok(Outcome::ok(format!("{} has {}.", repo.full, describe_release(&fields)), fields))
}
