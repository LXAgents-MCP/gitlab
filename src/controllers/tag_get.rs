//! Read one tag.
use super::prelude::*;
use crate::utils::text::short_str;

pub fn def() -> ToolDef {
    ToolDef {
        name: "tag_get",
        description: "Read one tag of a GitLab project: the commit it points at and, for an annotated tag, its message \
                      and date, and the release attached to it if there is one. Use it to find the commit behind a \
                      version tag. tag_check only says whether a tag exists.",
        input_schema: input(vec![repo_arg(), req("tag", string_arg("Tag name, e.g. 'v1.2.0'."))]),
        output_schema: output(vec![
            req("tag", string()),
            req("sha", string()),
            req("target", string()),
            req("annotated", boolean()),
            req("protected", boolean()),
            opt("message", string()),
            opt("date", string()),
            req("has_release", boolean()),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let tag = ref_name(arg(args, "tag"), "tag")?;

    let body = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/tags/{}", repo.path, ref_path(&tag)),
        Call::new(format!("read the tag '{tag}' of {}", repo.full)).explain(|status, _| {
            (status == 404).then(|| format!("There is no tag '{tag}' in {}. List them with tag_get_all.", repo.full))
        }),
    )
    .await?
    .body;

    let Some(sha) = at(&body, "commit.id").as_str() else {
        return Err(Refused::new("GitLab answered, but the body was not a tag this server recognizes."));
    };
    // For an annotated tag, `target` is the tag object and not the commit.
    let target = at(&body, "target").as_str().unwrap_or(sha);
    let annotated = target != sha;
    let annotated_only = |value: &Value| if annotated { value.clone() } else { Value::Null };
    Ok(Outcome::ok(
        format!(
            "Tag {tag} is {} and points at {}.",
            if annotated { "annotated" } else { "a lightweight tag" },
            short_str(sha)
        ),
        Fields::new()
            .set("tag", tag.clone())
            .set("sha", sha)
            .set("target", target)
            .set("annotated", annotated)
            .set("protected", truthy(at(&body, "protected")))
            .truthy("message", &annotated_only(at(&body, "message")))
            .truthy("date", &annotated_only(at(&body, "created_at")))
            .set("has_release", truthy(at(&body, "release"))),
    ))
}
