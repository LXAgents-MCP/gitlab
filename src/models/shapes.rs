//! GitLab's objects as this server reports them: a schema for a tool's output, and the function that
//! fills it. Kept apart from the tools so every tool that returns a project (or a commit or an issue) says
//! the same things about it, and a field GitLab renames is changed once.
use serde_json::Value;

use super::outcome::Fields;
use super::schemas::{Prop, array, boolean, num, obj, opt, req, string, strings};
use crate::utils::errors::Refused;
use crate::utils::js::{at, list, or, truthy};

/// Check that an answer is the kind of object the tool expected, before reading fields off it.
///
/// GitLab answers 2xx with a body of the shape documented, and a proxy or an outage can answer 2xx with
/// something else. `field` is a string field the object must have; `what` is what it should have been.
pub fn expect_body<'a>(body: &'a Value, field: &str, what: &str) -> Result<&'a Value, Refused> {
    match body {
        Value::Object(map) if map.get(field).is_some_and(Value::is_string) => Ok(body),
        _ => Err(Refused(format!(
            "GitLab answered, but the body was not {what} this server recognizes."
        ))),
    }
}

/* ------------------------------------------------------------------ projects */

/// What every tool that returns a project reports about it.
pub fn project_shape() -> Vec<Prop> {
    vec![
        req("id", num()),
        req("full_name", string()),
        opt("description", string()),
        req("html_url", string()),
        req("clone_url", string()),
        req("visibility", string()),
        opt("default_branch", string()),
        req("fork", boolean()),
        req("archived", boolean()),
        req("stars", num()),
        req("forks", num()),
        req("open_issues", num()),
        opt("license", string()),
        req("topics", strings()),
        opt("last_activity_at", string()),
        opt("parent", string()),
        opt("scheduled_for_deletion", string()),
    ]
}

fn zero() -> Value {
    Value::from(0)
}

/// A project as GitLab returns it, as this server reports it.
pub fn project_fields(project: &Value) -> Fields {
    let topics = match (at(project, "topics"), at(project, "tag_list")) {
        (Value::Array(t), _) => Value::Array(t.clone()),
        (_, Value::Array(t)) => Value::Array(t.clone()),
        _ => Value::Array(vec![]),
    };
    let deletion = or(
        at(project, "marked_for_deletion_on"),
        at(project, "marked_for_deletion_at"),
    );
    let default_branch = at(project, "default_branch");
    Fields::new()
        .raw("id", at(project, "id"))
        .raw("full_name", at(project, "path_with_namespace"))
        .truthy("description", at(project, "description"))
        .raw("html_url", at(project, "web_url"))
        .raw("clone_url", at(project, "http_url_to_repo"))
        .set(
            "visibility",
            or(at(project, "visibility"), &Value::from("unknown")).clone(),
        )
        .opt("default_branch", default_branch.as_str())
        .set("fork", truthy(at(project, "forked_from_project")))
        .set("archived", truthy(at(project, "archived")))
        .set("stars", or(at(project, "star_count"), &zero()).clone())
        .set("forks", or(at(project, "forks_count"), &zero()).clone())
        .set(
            "open_issues",
            or(at(project, "open_issues_count"), &zero()).clone(),
        )
        .truthy("license", at(project, "license.name"))
        .set("topics", topics)
        .truthy("last_activity_at", at(project, "last_activity_at"))
        .truthy(
            "parent",
            at(project, "forked_from_project.path_with_namespace"),
        )
        .opt(
            "scheduled_for_deletion",
            deletion.as_str().filter(|s| !s.is_empty()),
        )
}

fn text_of(fields: &Fields, key: &str) -> String {
    fields
        .get(key)
        .map(crate::utils::js::js_string)
        .unwrap_or_else(|| "undefined".into())
}

/// One line about a project, for a summary.
pub fn describe_project(fields: &Fields) -> String {
    let kind = if fields.get("fork").is_some_and(truthy) {
        "fork"
    } else {
        "project"
    };
    let archived = if fields.get("archived").is_some_and(truthy) {
        "archived "
    } else {
        ""
    };
    let branch = fields
        .get("default_branch")
        .filter(|v| truthy(v))
        .map(|b| format!(", default branch {}", crate::utils::js::js_string(b)))
        .unwrap_or_default();
    let deletion = fields
        .get("scheduled_for_deletion")
        .filter(|v| truthy(v))
        .map(|d| {
            format!(
                ", scheduled for deletion on {}",
                crate::utils::js::js_string(d)
            )
        })
        .unwrap_or_default();
    format!(
        "{} is a {} {archived}{kind}{branch}{deletion}.",
        text_of(fields, "full_name"),
        text_of(fields, "visibility")
    )
}

/* ------------------------------------------------------------------ releases */

/// What every tool that returns a release reports about it.
pub fn release_shape() -> Vec<Prop> {
    vec![
        req("tag", string()),
        opt("name", string()),
        req("upcoming", boolean()),
        opt("html_url", string()),
        opt("released_at", string()),
        opt("body", string()),
        req(
            "assets",
            array(obj(vec![req("name", string()), req("url", string())])),
        ),
    ]
}

/// A release as GitLab returns it. `assets` are the links attached to it and the source archives GitLab
/// makes, which have no name of their own.
pub fn release_fields(release: &Value) -> Fields {
    let mut assets: Vec<Value> = Vec::new();
    for link in list(at(release, "assets.links")) {
        let mut asset = serde_json::Map::new();
        if !at(link, "name").is_null() {
            asset.insert("name".into(), at(link, "name").clone());
        }
        let url = or(at(link, "direct_asset_url"), at(link, "url"));
        if !url.is_null() {
            asset.insert("url".into(), url.clone());
        }
        assets.push(Value::Object(asset));
    }
    for source in list(at(release, "assets.sources")) {
        let mut asset = serde_json::Map::new();
        asset.insert(
            "name".into(),
            Value::from(format!(
                "source.{}",
                crate::utils::js::js_string(at(source, "format"))
            )),
        );
        if !at(source, "url").is_null() {
            asset.insert("url".into(), at(source, "url").clone());
        }
        assets.push(Value::Object(asset));
    }
    Fields::new()
        .raw("tag", at(release, "tag_name"))
        .truthy("name", at(release, "name"))
        .set("upcoming", truthy(at(release, "upcoming_release")))
        .truthy("html_url", at(release, "_links.self"))
        .truthy("released_at", at(release, "released_at"))
        .truthy("body", at(release, "description"))
        .set("assets", Value::Array(assets))
}

/// One line about a release, for a summary.
pub fn describe_release(fields: &Fields) -> String {
    let upcoming = fields.get("upcoming").is_some_and(truthy);
    let tag = text_of(fields, "tag");
    let name = fields
        .get("name")
        .map(crate::utils::js::js_string)
        .unwrap_or_else(|| tag.clone());
    format!(
        "{} {name} (tag {tag})",
        if upcoming {
            "upcoming release"
        } else {
            "release"
        }
    )
}

/* ------------------------------------------------------------------ issues and comments */

/// Names out of a list GitLab sends either as plain strings (labels) or as objects (users). `key` is the
/// field that holds the name in an object: `name` or `username`.
pub fn names_in(value: &Value, key: &str) -> Vec<String> {
    list(value)
        .iter()
        .filter_map(|entry| match entry {
            Value::String(s) => Some(s.clone()),
            other => at(other, key).as_str().map(str::to_string),
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// What `issue_read` reports about an issue.
pub fn issue_shape() -> Vec<Prop> {
    vec![
        req("number", num()),
        req("title", string()),
        req("state", string()),
        opt("author", string()),
        req("labels", strings()),
        req("assignees", strings()),
        opt("milestone", string()),
        req("comment_count", num()),
        req("locked", boolean()),
        req("confidential", boolean()),
        req("html_url", string()),
        opt("created_at", string()),
        opt("updated_at", string()),
        opt("closed_at", string()),
        opt("body", string()),
    ]
}

/// An issue as GitLab returns it. `number` is its `iid`, the number in the project that people write as
/// #42; `id` is global and is not what any tool takes.
pub fn issue_fields(issue: &Value) -> Fields {
    Fields::new()
        .raw("number", at(issue, "iid"))
        .raw("title", at(issue, "title"))
        .raw("state", at(issue, "state"))
        .truthy("author", at(issue, "author.username"))
        .set("labels", names_in(at(issue, "labels"), "name"))
        .set("assignees", names_in(at(issue, "assignees"), "username"))
        .truthy("milestone", at(issue, "milestone.title"))
        .set(
            "comment_count",
            or(at(issue, "user_notes_count"), &zero()).clone(),
        )
        .set("locked", truthy(at(issue, "discussion_locked")))
        .set("confidential", truthy(at(issue, "confidential")))
        .raw("html_url", at(issue, "web_url"))
        .truthy("created_at", at(issue, "created_at"))
        .truthy("updated_at", at(issue, "updated_at"))
        .truthy("closed_at", at(issue, "closed_at"))
        .truthy("body", at(issue, "description"))
}

/// A comment, as `issue_read` and `pr_read` list it.
pub fn comment_shape() -> Value {
    obj(vec![
        req("id", num()),
        opt("author", string()),
        opt("created_at", string()),
        req("body", string()),
    ])
}

/// The comments people wrote, out of a list of notes. GitLab keeps its own account of what happened (a
/// label added, a branch pushed) in the same list, marked `system`; those are not comments.
pub fn comments_from(notes: &Value) -> Vec<Value> {
    list(notes)
        .iter()
        .filter(|note| at(note, "system") != &Value::Bool(true))
        .map(|note| {
            Fields::new()
                .raw("id", at(note, "id"))
                .truthy("author", at(note, "author.username"))
                .truthy("created_at", at(note, "created_at"))
                .set("body", or(at(note, "body"), &Value::from("")).clone())
                .into_value()
        })
        .collect()
}

/* ------------------------------------------------------------------ jobs */

/// What the workflow tools report about a job of a pipeline.
pub fn job_shape() -> Vec<Prop> {
    vec![
        req("id", num()),
        req("name", string()),
        opt("stage", string()),
        req("status", string()),
        req("allow_failure", boolean()),
        opt("failure_reason", string()),
        opt("duration", num()),
    ]
}

/// A job as GitLab returns it. `failure_reason` is GitLab's word for why a job failed, and
/// `allow_failure` says the pipeline does not fail with it. A job has no steps; the log says where.
pub fn job_fields(job: &Value) -> Fields {
    let duration = at(job, "duration")
        .as_f64()
        .map(|d| Value::from(d.round() as i64));
    Fields::new()
        .raw("id", at(job, "id"))
        .set("name", or(at(job, "name"), &Value::from("")).clone())
        .truthy("stage", at(job, "stage"))
        .set(
            "status",
            or(at(job, "status"), &Value::from("unknown")).clone(),
        )
        .set("allow_failure", truthy(at(job, "allow_failure")))
        .truthy("failure_reason", at(job, "failure_reason"))
        .opt("duration", duration)
}
