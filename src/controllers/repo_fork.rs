//! Fork a project.
use super::prelude::*;
use crate::models::address::{has_reserved_suffix, is_namespace_path, is_segment};
use crate::models::shapes::{project_fields, project_shape};

pub fn def() -> ToolDef {
    let mut out = project_shape();
    out.push(opt("import_status", string()));
    ToolDef {
        name: "repo_fork",
        description: "Fork a GitLab project into the token owner's namespace or into a group. GitLab copies it in the \
                      background, so the fork may take a moment to be readable. Use it to work on a repository the \
                      owner cannot push to.",
        input_schema: input(vec![
            repo_arg(),
            opt(
                "group",
                string_arg("Full path of the group or namespace to fork into. Omit to fork into the token owner's namespace."),
            ),
            opt("name", string_arg("Name for the fork. Omit to keep the source's name.")),
            opt("path", string_arg("URL path for the fork. Omit to keep the source's path.")),
        ]),
        output_schema: output(out),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let group = optional_text(arg(args, "group"), "group")?;
    if let Some(group) = &group
        && !is_namespace_path(group) {
            return Err(Refused(format!(
                "`group` '{group}' is not a valid GitLab namespace path. Give the full path, such as 'my-group' or \
                 'my-group/sub-group'; omit it to fork into the token owner's namespace."
            )));
        }
    let name = optional_text(arg(args, "name"), "name")?;
    let path = optional_text(arg(args, "path"), "path")?;
    if let Some(path) = &path
        && (!is_segment(path) || has_reserved_suffix(path)) {
            return Err(Refused(format!(
                "`path` '{path}' is not a valid GitLab project path. Use letters, digits, '_', '-' and '.', \
                 not starting with '-' or '.' and not ending in '.git' or '.atom'."
            )));
        }

    let mut request = serde_json::Map::new();
    for (key, value) in [("namespace_path", group), ("name", name), ("path", path)] {
        if let Some(value) = value {
            request.insert(key.into(), value.into());
        }
    }
    let body =
        gitlab(ctx, "POST", &format!("{}/fork", repo.path), Call::new(format!("fork {}", repo.full)).json(Value::Object(request)))
            .await?
            .body;

    let project = expect_body(&body, "path_with_namespace", "a project")?;
    let fields = project_fields(project);
    let full_name = fields.get("full_name").map(js_string).unwrap_or_else(|| "undefined".into());
    Ok(Outcome::ok(
        format!(
            "Forking {} into {full_name} has started. GitLab finishes the copy in the \
             background, so give it a moment before reading it.",
            repo.full
        ),
        fields.opt("import_status", at(project, "import_status").as_str()),
    ))
}
