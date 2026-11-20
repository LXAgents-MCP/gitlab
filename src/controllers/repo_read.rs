//! Read one project.
use super::prelude::*;
use crate::models::shapes::{describe_project, project_fields, project_shape};

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_read",
        description: "Read one GitLab project: description, visibility, default branch, star, fork and open issue \
                      counts, license, topics, last activity and parent. Use it to learn about a repository before \
                      acting on it.",
        input_schema: input(vec![repo_arg()]),
        output_schema: output(project_shape()),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let body = gitlab(ctx, "GET", &repo.path, Call::new(format!("read the project {}", repo.full)).query("license", Some(true)))
        .await?
        .body;

    let fields = project_fields(expect_body(&body, "path_with_namespace", "a project")?);
    Ok(Outcome::ok(describe_project(&fields), fields))
}
