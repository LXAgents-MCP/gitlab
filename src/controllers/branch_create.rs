//! Create a branch.
use super::prelude::*;

pub fn def() -> ToolDef {
    ToolDef {
        name: "branch_create",
        description: "Create a branch in a GitLab project, pointing at a branch, tag or commit. Use it to start \
                      work off the default branch or off another branch. Fails if the name is taken.",
        input_schema: input(vec![
            repo_arg(),
            req("branch", string_arg("Name of the new branch, e.g. 'feat/login'.")),
            opt("from", string_arg("Branch, tag or commit sha to start from. Default: the project's default branch.")),
        ]),
        output_schema: output(vec![req("branch", string()), req("sha", string()), opt("from", string())]),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let branch = ref_name(arg(args, "branch"), "branch")?;
    let from = optional_ref(arg(args, "from"), "from")?;

    // GitLab wants the starting point named, so the default branch is looked up when none is given.
    let source = match from {
        Some(from) => from,
        None => {
            let answer = gitlab(ctx, "GET", &repo.path, Call::new(format!("find the default branch of {}", repo.full))).await?;
            match nonempty_str(at(&answer.body, "default_branch")) {
                Some(found) => found.to_string(),
                None => {
                    return Err(Refused(format!(
                        "{} has no default branch, so there is nothing to branch from. Create its first commit with file_create, or pass `from`.",
                        repo.full
                    )));
                }
            }
        }
    };

    let exists = regex::Regex::new(r"(?i)already exists").expect("pattern");
    let missing = regex::Regex::new(r"(?i)invalid reference|not found|does not exist").expect("pattern");
    let answer = gitlab(
        ctx,
        "POST",
        &format!("{}/repository/branches", repo.path),
        Call::new(format!("create the branch '{branch}' in {}", repo.full))
            .query("branch", Some(&branch))
            .query("ref", Some(&source))
            .explain(|status, detail| {
                if status == 400 && exists.is_match(detail) {
                    return Some(format!(
                        "GitLab could not create the branch '{branch}' (400){detail}. A branch with that name already exists in {}; pick another name.",
                        repo.full
                    ));
                }
                if status == 400 && missing.is_match(detail) {
                    return Some(format!(
                        "GitLab could not create the branch '{branch}' (400){detail}. '{source}' is not a branch, tag or commit of {}.",
                        repo.full
                    ));
                }
                None
            }),
    )
    .await?;

    let body = expect_body(&answer.body, "name", "a branch")?;
    let sha = or(at(body, "commit.id"), &json!("")).clone();
    let name = js_string(at(body, "name"));
    Ok(Outcome::ok(
        format!("Created branch {name} at {} in {}, from {source}.", short(&sha), repo.full),
        Fields::new().set("branch", name).set("sha", sha).set("from", source),
    ))
}
