//! Change who an issue is assigned to.
use super::prelude::*;
use crate::models::shapes::names_in;
use crate::services::users::{bare_username, find_users};

const ACTIONS: &[&str] = &["add", "remove", "set"];

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_assign",
        description: "Add, remove or replace the assignees of an issue in a GitLab project. An unknown username refuses the \
                      call before anything changes. A free plan keeps one assignee per issue; the result lists any person \
                      GitLab did not apply.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Issue number in the project.")),
            req("assignees", string_list_arg("GitLab usernames. At most 10. With action set, [] unassigns everyone.")),
            opt(
                "action",
                enum_arg(
                    ACTIONS,
                    "add (default) keeps the current assignees and adds these; remove takes these off; set replaces all.",
                ),
            ),
        ]),
        output_schema: output(vec![req("number", num()), req("assignees", strings()), opt("not_applied", strings())]),
        annotations: UPDATE,
    }
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let action = one_of(arg(args, "action"), "action", ACTIONS, Some("add"))?.unwrap_or_default();
    let requested: Vec<String> = match text_list(arg(args, "assignees"), "assignees", ListOptions { required: false, max: 10 })? {
        Some(names) => names.iter().map(|n| bare_username(n)).collect(),
        None => {
            return Err(Refused::new(
                "`assignees` is required and must be a list of usernames; with action set, [] unassigns everyone.",
            ));
        }
    };
    if requested.is_empty() && action != "set" {
        return Err(Refused::new(
            "`assignees` must list at least one username. Only action set may be empty, to unassign everyone.",
        ));
    }

    let path = format!("{}/issues/{number}", repo.path);
    let what = format!("{action} assignees on #{number} of {}", repo.full);

    // Changing one person means sending the whole list, so add and remove start from who is there now.
    let mut issue = Value::Null;
    let mut before: Vec<(i64, String)> = Vec::new();
    if action != "set" {
        issue = gitlab(ctx, "GET", &path, Call::new(format!("read the assignees of #{number} of {}", repo.full))).await?.body;
        expect_body(&issue, "web_url", "an issue")?;
        before = list(at(&issue, "assignees"))
            .iter()
            .filter_map(|user| as_int(at(user, "id")).map(|id| (id, js_string(at(user, "username")))))
            .collect();
    }

    let next: Vec<i64> = if action == "remove" {
        before.iter().filter(|(_, name)| !requested.iter().any(|r| same(r, name))).map(|(id, _)| *id).collect()
    } else {
        let people = if requested.is_empty() { Vec::new() } else { find_users(ctx, &requested, &what).await? };
        let mut ids: Vec<i64> = if action == "add" { before.iter().map(|(id, _)| *id).collect() } else { Vec::new() };
        for person in people {
            ids.push(person.id);
        }
        let mut unique = Vec::new();
        for id in ids {
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        unique
    };

    let unchanged = action != "set" && next.len() == before.len() && next.iter().all(|id| before.iter().any(|(b, _)| b == id));
    if !unchanged {
        // GitLab clears every assignee when it is sent the id 0.
        let ids = if next.is_empty() { vec![0] } else { next.clone() };
        issue = gitlab(ctx, "PUT", &path, Call::new(what.clone()).json(json!({ "assignee_ids": ids }))).await?.body;
    }

    expect_body(&issue, "web_url", "an issue")?;
    let now = names_in(at(&issue, "assignees"), "username");
    let has = |names: &[String], name: &str| names.iter().any(|entry| same(entry, name));
    let was_assigned: Vec<String> = before.iter().map(|(_, name)| name.clone()).collect();
    let not_applied: Vec<String> = if action == "remove" {
        requested.iter().filter(|name| !has(&was_assigned, name) || has(&now, name)).cloned().collect()
    } else {
        requested.iter().filter(|name| !has(&now, name)).cloned().collect()
    };

    let note = if not_applied.is_empty() {
        String::new()
    } else if action == "remove" {
        format!(" Not removed: {}. They were not assigned, or GitLab kept them.", not_applied.join(", "))
    } else {
        format!(" GitLab did not assign: {}. A free plan keeps one assignee per issue.", not_applied.join(", "))
    };
    let names = if now.is_empty() { "none".to_string() } else { now.join(", ") };
    Ok(Outcome::ok(
        format!("Assignees of #{number} in {}: {names}.{note}", repo.full),
        Fields::new()
            .set("number", number)
            .set("assignees", now)
            .opt("not_applied", (!not_applied.is_empty()).then_some(not_applied)),
    ))
}
