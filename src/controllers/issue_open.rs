//! Open an issue.
use super::prelude::*;
use crate::models::shapes::names_in;
use crate::services::users::find_users;

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_open",
        description: "Open a new issue in a GitLab project, with optional body, labels, assignees and milestone. GitLab \
                      creates a label that does not exist yet, so a misspelled label becomes a new one. A free plan keeps \
                      one assignee per issue. The result lists what was actually applied.",
        input_schema: input(vec![
            repo_arg(),
            req("title", string_arg("Issue title.")),
            opt("body", string_arg("Issue body, in Markdown.")),
            opt("labels", string_list_arg("Label names to apply. They may not contain a comma.")),
            opt("assignees", string_list_arg("GitLab usernames to assign. An unknown username refuses the call.")),
            opt(
                "milestone",
                int_arg("Milestone id (the id GitLab gives the milestone, not its title or its number in the project)."),
            ),
        ]),
        output_schema: output(vec![
            req("number", num()),
            req("title", string()),
            req("state", string()),
            req("html_url", string()),
            req("labels", strings()),
            req("assignees", strings()),
        ]),
        annotations: WRITE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let title = required_text(arg(args, "title"), "title")?;
    let body = raw_text(arg(args, "body"), "body")?;
    let labels = label_list(arg(args, "labels"), "labels", ListOptions::default())?;
    let assignees = text_list(arg(args, "assignees"), "assignees", ListOptions { required: false, max: 10 })?;
    let milestone = optional_int(arg(args, "milestone"), "milestone", IntRange::min(1))?;

    let people = match &assignees {
        Some(names) if !names.is_empty() => find_users(ctx, names, &format!("assign an issue in {}", repo.full)).await?,
        _ => Vec::new(),
    };

    let mut json = json!({ "title": title });
    if let Some(body) = &body {
        json["description"] = Value::from(body.as_str());
    }
    if let Some(labels) = labels.as_ref().filter(|l| !l.is_empty()) {
        json["labels"] = Value::from(labels.join(","));
    }
    if !people.is_empty() {
        json["assignee_ids"] = Value::from(people.iter().map(|p| p.id).collect::<Vec<_>>());
    }
    if let Some(milestone) = milestone {
        json["milestone_id"] = Value::from(milestone);
    }
    let created = gitlab(ctx, "POST", &format!("{}/issues", repo.path), Call::new(format!("open an issue in {}", repo.full)).json(json))
        .await?
        .body;

    expect_body(&created, "web_url", "an issue")?;
    let applied_labels = names_in(at(&created, "labels"), "name");
    let applied_assignees = names_in(at(&created, "assignees"), "username");
    let same = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
    let mut dropped: Vec<String> = labels
        .iter()
        .flatten()
        .filter(|name| !applied_labels.iter().any(|now| same(now, name)))
        .map(|name| format!("label {name}"))
        .collect();
    dropped.extend(
        people
            .iter()
            .filter(|person| !applied_assignees.iter().any(|now| same(now, &person.username)))
            .map(|person| format!("assignee {}", person.username)),
    );
    let note = if dropped.is_empty() {
        String::new()
    } else {
        format!(
            " GitLab did not apply: {}. A free plan keeps one assignee, and a token without permission to create labels cannot add a new one.",
            dropped.join(", ")
        )
    };
    Ok(Outcome::ok(
        format!(
            "Opened issue #{} in {}: {}.{note}",
            js_string(at(&created, "iid")),
            repo.full,
            js_string(at(&created, "title"))
        ),
        Fields::new()
            .raw("number", at(&created, "iid"))
            .raw("title", at(&created, "title"))
            .raw("state", at(&created, "state"))
            .raw("html_url", at(&created, "web_url"))
            .set("labels", applied_labels)
            .set("assignees", applied_assignees),
    ))
}
