//! Change a project's settings.
use super::prelude::*;
use crate::models::address::{has_reserved_suffix, is_segment};
use crate::models::shapes::{describe_project, project_fields, project_shape};

const VISIBILITIES: &[&str] = &["private", "internal", "public"];

pub fn def() -> ToolDef {
    ToolDef {
        name: "repo_update",
        description: "Change a GitLab project's settings: rename it or move its URL path, set its description, make it \
                      private, internal or public, change the default branch, archive or unarchive it, or switch issues \
                      on or off. Pass only what should change; at least one setting is required.",
        input_schema: input(vec![
            repo_arg(),
            opt("name", string_arg("New display name. It does not change the URL; use `path` for that.")),
            opt(
                "path",
                string_arg("New URL path: letters, digits, '_', '-' and '.'. Changing it changes the project's address."),
            ),
            opt("description", string_arg("New description. Pass an empty string to clear it.")),
            opt("visibility", enum_arg(VISIBILITIES, "private, internal or public.")),
            opt("default_branch", string_arg("Branch to make the default. It must already exist.")),
            opt("archived", bool_arg("true archives the project (read-only), false unarchives it.")),
            opt("has_issues", bool_arg("Whether the issue tracker is enabled.")),
        ]),
        output_schema: output(project_shape()),
        annotations: UPDATE,
    }
}

/// One call that answers with the project; once it has, later calls use its id, which a new path cannot move.
async fn step(
    ctx: &Ctx,
    target: &mut String,
    method: &str,
    suffix: &str,
    what: String,
    body: Option<Value>,
) -> Result<Value, Refused> {
    let answer = gitlab(ctx, method, &format!("{target}{suffix}"), Call::new(what).json_opt(body)).await?.body;
    let project = expect_body(&answer, "path_with_namespace", "a project")?.clone();
    *target = format!("/projects/{}", js_string(at(&project, "id")));
    Ok(project)
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;

    let name = optional_text(arg(args, "name"), "name")?;
    let path = optional_text(arg(args, "path"), "path")?;
    if let Some(path) = &path
        && (!is_segment(path) || has_reserved_suffix(path)) {
            return Err(Refused(format!(
                "`path` '{path}' is not a valid GitLab project path. Use letters, digits, '_', '-' and '.', \
                 not starting with '-' or '.' and not ending in '.git' or '.atom'."
            )));
        }
    let description = raw_text(arg(args, "description"), "description")?;
    let visibility = one_of(arg(args, "visibility"), "visibility", VISIBILITIES, None)?;
    let default_branch = match args.get("default_branch") {
        None => None,
        Some(value) => Some(ref_name(value, "default_branch")?),
    };
    let archived = maybe_flag(arg(args, "archived"), "archived")?;
    let has_issues = maybe_flag(arg(args, "has_issues"), "has_issues")?;

    let changed: Vec<&str> = [
        ("name", name.is_some()),
        ("path", path.is_some()),
        ("description", description.is_some()),
        ("visibility", visibility.is_some()),
        ("default_branch", default_branch.is_some()),
        ("archived", archived.is_some()),
        ("has_issues", has_issues.is_some()),
    ]
    .into_iter()
    .filter_map(|(key, given)| given.then_some(key))
    .collect();
    if changed.is_empty() {
        return Err(Refused::new(
            "Nothing to change: pass at least one of name, path, description, visibility, default_branch, \
             archived or has_issues.",
        ));
    }

    let mut settings = serde_json::Map::new();
    for (key, value) in [
        ("name", name),
        ("path", path),
        ("description", description),
        ("visibility", visibility),
        ("default_branch", default_branch),
        ("issues_access_level", has_issues.map(|on| if on { "enabled".to_string() } else { "disabled".to_string() })),
    ] {
        if let Some(value) = value {
            settings.insert(key.into(), value.into());
        }
    }
    let has_settings = !settings.is_empty();

    // An archived project is read-only, so it is unarchived before its settings change and archived
    // after. Once the first answer names the project, later calls use its id, which a new path cannot move.
    let mut target = repo.path.clone();
    let mut project = Value::Null;
    if archived == Some(false) {
        project = step(ctx, &mut target, "POST", "/unarchive", format!("unarchive the project {}", repo.full), None).await?;
    }
    if has_settings {
        match step(ctx, &mut target, "PUT", "", format!("update the project {}", repo.full), Some(Value::Object(settings))).await
        {
            Ok(answer) => project = answer,
            Err(refused) if archived == Some(false) => {
                return Err(Refused(format!("{} The project had already been unarchived.", refused.0)));
            }
            Err(refused) => return Err(refused),
        }
    }
    if archived == Some(true) {
        match step(ctx, &mut target, "POST", "/archive", format!("archive the project {}", repo.full), None).await {
            Ok(answer) => project = answer,
            Err(refused) if has_settings => {
                return Err(Refused(format!("{} The other settings had already been changed.", refused.0)));
            }
            Err(refused) => return Err(refused),
        }
    }

    let fields = project_fields(&project);
    let full_name = fields.get("full_name").map(js_string).unwrap_or_else(|| "undefined".into());
    Ok(Outcome::ok(format!("Updated {full_name}: {}. {}", changed.join(", "), describe_project(&fields)), fields))
}
