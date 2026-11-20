//! Create a release.
use super::prelude::*;
use crate::models::shapes::{describe_release, release_fields, release_shape};

pub fn def() -> ToolDef {
    ToolDef {
        name: "release_create",
        description: "Create a release for a tag in a GitLab project, creating the tag from a branch or commit if it \
                      does not exist. A release date in the future makes it an upcoming release. This is also how a tag \
                      is created. GitLab has no draft or prerelease flag and does not write notes.",
        input_schema: input(vec![
            repo_arg(),
            req("tag", string_arg("Tag for the release, e.g. 'v1.2.0'. Created if it does not exist.")),
            opt(
                "target",
                string_arg("Branch or commit sha to create the tag from. Default: the default branch. Ignored if the tag exists."),
            ),
            opt("name", string_arg("Release title. Default: the tag.")),
            opt("body", string_arg("Release notes, in Markdown.")),
            opt(
                "released_at",
                string_arg(
                    "ISO 8601 date of the release. A future date makes it an upcoming release; a past one a historical release.",
                ),
            ),
        ]),
        output_schema: output(release_shape()),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let tag = ref_name(arg(args, "tag"), "tag")?;
    let target = match arg(args, "target") {
        Value::Null => None,
        target => Some(ref_name(target, "target")?),
    };
    let name = optional_text(arg(args, "name"), "name")?;
    let body = raw_text(arg(args, "body"), "body")?;
    let released_at = optional_date(arg(args, "released_at"), "released_at")?;

    // GitLab wants the starting point named whenever the tag might not exist yet, and ignores it when it does.
    let reference = branch_or_default(ctx, &repo, target).await?;
    let mut request = serde_json::Map::new();
    request.insert("tag_name".into(), tag.clone().into());
    request.insert("ref".into(), reference.into());
    for (key, value) in [("name", name), ("description", body), ("released_at", released_at)] {
        if let Some(value) = value {
            request.insert(key.into(), value.into());
        }
    }

    let exists = regex::Regex::new(r"(?i)already exists").expect("pattern");
    let created = gitlab(
        ctx,
        "POST",
        &format!("{}/releases", repo.path),
        Call::new(format!("create a release for the tag '{tag}' in {}", repo.full)).json(Value::Object(request)).explain(
            |status, detail| {
                (status == 409 || (status == 400 && exists.is_match(detail))).then(|| {
                    format!(
                        "GitLab could not create a release for '{tag}' in {} ({status}){detail}. A release for that tag \
                         already exists; read it with release_read, or use another tag.",
                        repo.full
                    )
                })
            },
        ),
    )
    .await?
    .body;

    expect_body(&created, "tag_name", "a release")?;
    let fields = release_fields(&created);
    Ok(Outcome::ok(format!("Created {} in {}.", describe_release(&fields), repo.full), fields))
}
