//! Say whether a tag exists.
use super::prelude::*;
use crate::services::failure::api_detail;
use crate::utils::text::short_str;

pub fn def() -> ToolDef {
    ToolDef {
        name: "tag_check",
        description: "Say whether a tag exists in a GitLab project, and if so what it points at and whether it is \
                      annotated. A missing tag is an answer, not an error. Use it before creating a release or a tag.",
        input_schema: input(vec![repo_arg(), req("tag", string_arg("Tag name, e.g. 'v1.2.0'."))]),
        output_schema: output(vec![
            req("exists", boolean()),
            req("tag", string()),
            opt("sha", string()),
            opt("annotated", boolean()),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let tag = ref_name(arg(args, "tag"), "tag")?;

    // A tag that is not there is the answer to this call, so a 404 is returned and not refused -
    // unless it is the project that is missing.
    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/tags/{}", repo.path, ref_path(&tag)),
        Call::new(format!("look for the tag '{tag}' in {}", repo.full)).allow(&[404]),
    )
    .await?;

    if answer.status == 404 {
        let detail = api_detail(&answer.body);
        if detail.to_lowercase().contains("project") {
            return Err(Refused(format!(
                "GitLab could not find the project {} (404){detail}. Check the spelling. GitLab also answers 404 for a \
                 private project the token cannot see.",
                repo.full
            )));
        }
        return Ok(Outcome::ok(
            format!("Tag {tag} does not exist in {}.", repo.full),
            Fields::new().set("exists", false).set("tag", tag.clone()),
        ));
    }

    let Some(sha) = at(&answer.body, "commit.id").as_str() else {
        return Err(Refused::new("GitLab answered, but the body was not a tag this server recognizes."));
    };
    let annotated = at(&answer.body, "target").as_str().is_some_and(|target| target != sha);
    Ok(Outcome::ok(
        format!(
            "Tag {tag} exists in {} ({}) and points at {}.",
            repo.full,
            if annotated { "annotated" } else { "lightweight" },
            short_str(sha)
        ),
        Fields::new().set("exists", true).set("tag", tag.clone()).set("sha", sha).set("annotated", annotated),
    ))
}
