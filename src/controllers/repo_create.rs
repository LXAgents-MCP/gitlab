//! Create a project (a repository) on GitLab.
//!
//! Moved from the `cli` server, where it was `create_gitlab_repo`. The arguments, the defaults
//! and the wording of its refusals are unchanged.
use super::prelude::*;
use crate::models::address::{encode_component, has_reserved_suffix, is_namespace_path, is_segment};

const VISIBILITIES: &[&str] = &["private", "internal", "public"];

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_create",
        description: "Create a new repository (a project) on GitLab.com and return its URLs. Use this when the \
                      owner asks to start a new GitLab repository, in their own namespace or in a group. \
                      Private unless told otherwise. It only creates: it does not clone, push to, change or \
                      delete anything. The token is read from the GITLAB_API_KEY environment variable of the \
                      server, never from an argument.",
        input_schema: input(vec![
            req(
                "name",
                string_arg(
                    "Repository name, which is also its URL path: letters, digits, '_', '-' and '.', \
                     not starting with '-' or '.'. Example: 'my-new-repo'.",
                ),
            ),
            opt(
                "group",
                string_arg(
                    "Full path of the group or subgroup to create it in. Omit to create it in the \
                     personal namespace that owns the token. Example: 'lxagents-mcp' or 'lxagents-mcp/tools'.",
                ),
            ),
            opt("description", string_arg("One-line description shown on the repository.")),
            opt(
                "visibility",
                enum_arg(
                    VISIBILITIES,
                    "Default 'private'. 'internal' is visible to any signed-in GitLab.com user. Pass \
                     'public' only when the owner asked for a public repository.",
                ),
            ),
            opt(
                "initialize_with_readme",
                bool_arg(
                    "Default true: create an initial commit with a README, so the repository can be \
                     cloned straight away. Pass false for a truly empty repository.",
                ),
            ),
        ]),
        output_schema: output(vec![
            req("name", string()),
            req("full_name", string()),
            req("html_url", string()),
            req("clone_url", string()),
            req("visibility", string()),
            opt("default_branch", string()),
        ]),
        annotations: WRITE,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Group,
    Project,
}

/// The wording of the statuses a create can meet, which names the next step for each. `None` leaves a
/// 401 to the general wording.
fn explain(status: u16, detail: &str, name: &str, group: Option<&str>, place: &str, step: Step) -> Option<String> {
    match status {
        401 => None,
        404 if step == Step::Group => Some(format!(
            "GitLab could not find the group '{}' (404), or the token cannot see it. Check \
             the full path, or omit `group` to create the repository in the token owner's namespace.",
            group.unwrap_or("undefined")
        )),
        403 => Some(format!(
            "GitLab refused the request (403){detail}. The token needs the `api` scope, and its \
             owner must be allowed to create projects {place}."
        )),
        400 | 409 | 422 => Some(format!(
            "GitLab could not create '{name}' ({status}){detail}. The most common cause is that \
             a project with that name or path already exists {place}; pick another name."
        )),
        _ => Some(format!("GitLab answered {status}{detail}.")),
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let name = required_text(arg(args, "name"), "name")?;
    if !is_segment(&name) || has_reserved_suffix(&name) {
        return Err(Refused(format!(
            "`name` '{name}' is not a valid GitLab repository name. Use letters, digits, '_', \
             '-' and '.', not starting with '-' or '.' and not ending in '.git' or '.atom'."
        )));
    }

    let group = optional_text(arg(args, "group"), "group")?;
    if let Some(group) = &group
        && !is_namespace_path(group) {
            return Err(Refused(format!(
                "`group` '{group}' is not a valid GitLab group path. Give the full path, such as \
                 'my-group' or 'my-group/sub-group'; omit it to create the repository in the token \
                 owner's namespace."
            )));
        }

    let description = optional_text(arg(args, "description"), "description")?;
    let visibility = optional_text(arg(args, "visibility"), "visibility")?.unwrap_or_else(|| "private".into());
    if !VISIBILITIES.contains(&visibility.as_str()) {
        return Err(Refused(format!("`visibility` must be one of {}, got '{visibility}'.", VISIBILITIES.join(", "))));
    }
    let readme = optional_flag(arg(args, "initialize_with_readme"), "initialize_with_readme", true)?;

    let place = match &group {
        Some(group) if !group.is_empty() => format!("in '{group}'"),
        _ => "in the token owner's namespace".to_string(),
    };

    // A project is created in a group by the group's id, not its path, so look it up first.
    let mut namespace_id = None;
    if let Some(group) = &group {
        let found = gitlab(
            ctx,
            "GET",
            &format!("/groups/{}", encode_component(group)),
            Call::new(format!("find the group '{group}'"))
                .explain(|code, detail| explain(code, detail, &name, Some(group), &place, Step::Group)),
        )
        .await?
        .body;
        if !is_int(at(&found, "id")) {
            return Err(Refused(format!(
                "GitLab answered 200 for the group '{group}', but without an id this server recognizes."
            )));
        }
        namespace_id = Some(at(&found, "id").clone());
    }

    let mut request = json!({
        "name": name,
        "path": name,
        "visibility": visibility,
        "initialize_with_readme": readme,
    });
    if let Some(id) = namespace_id {
        request["namespace_id"] = id;
    }
    if let Some(description) = description {
        request["description"] = Value::String(description);
    }
    let body = gitlab(
        ctx,
        "POST",
        "/projects",
        Call::new(format!("create the project '{name}' {place}"))
            .json(request)
            .explain(|code, detail| explain(code, detail, &name, group.as_deref(), &place, Step::Project)),
    )
    .await?
    .body;

    if !at(&body, "web_url").is_string() {
        return Err(Refused::new("GitLab answered 201, but the body was not a project this server recognizes."));
    }

    let created = match at(&body, "visibility") {
        Value::Null => Value::String(visibility.clone()),
        value => value.clone(),
    };
    let short_name = match at(&body, "path") {
        Value::Null => at(&body, "name").clone(),
        value => value.clone(),
    };
    Ok(Outcome::ok(
        format!("Created {} repository {} on GitLab.", js_string(&created), js_string(at(&body, "path_with_namespace"))),
        Fields::new()
            .set("name", short_name)
            .raw("full_name", at(&body, "path_with_namespace"))
            .raw("html_url", at(&body, "web_url"))
            .raw("clone_url", at(&body, "http_url_to_repo"))
            .set("visibility", created)
            .opt("default_branch", at(&body, "default_branch").as_str()),
    ))
}
