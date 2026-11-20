//! Read one branch.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_read",
        description: "Read one branch of a GitLab project: the commit at its tip, that commit's title and date, whether \
                      the branch is protected and whether it is the default. Use it to find a branch's current sha.",
        input_schema: input(vec![repo_arg(), req("branch", string_arg("Branch name, e.g. 'main' or 'feat/login'."))]),
        output_schema: output(vec![
            req("name", string()),
            req("sha", string()),
            req("protected", boolean()),
            req("default", boolean()),
            opt("message", string()),
            opt("date", string()),
        ]),
        annotations: READ,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let branch = ref_name(arg(args, "branch"), "branch")?;

    let answer = gitlab(
        ctx,
        "GET",
        &format!("{}/repository/branches/{}", repo.path, ref_path(&branch)),
        Call::new(format!("read the branch '{branch}' of {}", repo.full)),
    )
    .await?;

    let body = expect_body(&answer.body, "name", "a branch")?;
    let sha = or(at(body, "commit.id"), &json!("")).clone();
    let title = at(body, "commit.title");
    let message = if title.is_null() { Value::from(first_line(at(body, "commit.message"))) } else { title.clone() };
    let date = at(body, "commit.committed_date");
    let name = js_string(at(body, "name"));
    let default = truthy(at(body, "default"));
    let protected = truthy(at(body, "protected"));
    Ok(Outcome::ok(
        format!(
            "Branch {name} of {} is at {}{}{}.",
            repo.full,
            short(&sha),
            if default { ", the default" } else { "" },
            if protected { " and is protected" } else { "" }
        ),
        Fields::new()
            .set("name", name)
            .set("sha", sha)
            .set("protected", protected)
            .set("default", default)
            .truthy("message", &message)
            .truthy("date", date),
    ))
}
