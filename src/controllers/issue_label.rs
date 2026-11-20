//! Change the labels on an issue.
use super::prelude::*;
use crate::models::shapes::names_in;

const ACTIONS: &[&str] = &["add", "remove", "set"];

pub fn def() -> ToolDef {
    ToolDef {
        name: "issue_label",
        description: "Add, remove or replace the labels on an issue in a GitLab project. GitLab creates a label that does \
                      not exist yet, so a misspelled label becomes a new one. The result lists the labels now on the issue \
                      and any requested label that is not as asked.",
        input_schema: input(vec![
            repo_arg(),
            req("number", int_arg("Issue number in the project.")),
            req(
                "labels",
                string_list_arg(
                    "Label names, e.g. ['bug', 'good first issue']. They may not contain a comma. With action set, [] removes every label.",
                ),
            ),
            opt(
                "action",
                enum_arg(ACTIONS, "add (default) keeps the current labels and adds these; remove takes these off; set replaces all."),
            ),
        ]),
        output_schema: output(vec![req("number", num()), req("labels", strings()), opt("not_applied", strings())]),
        annotations: UPDATE,
    }
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let number = required_int(arg(args, "number"), "number", IntRange::min(1))?;
    let action = one_of(arg(args, "action"), "action", ACTIONS, Some("add"))?.unwrap_or_default();
    let Some(labels) = label_list(arg(args, "labels"), "labels", ListOptions::default())? else {
        return Err(Refused::new(
            "`labels` is required and must be a list of label names; with action set, [] removes every label.",
        ));
    };
    if labels.is_empty() && action != "set" {
        return Err(Refused::new(
            "`labels` must list at least one label. Only action set may be empty, to remove every label.",
        ));
    }

    let names = labels.join(",");
    let json = match action.as_str() {
        "add" => json!({ "add_labels": names }),
        "remove" => json!({ "remove_labels": names }),
        _ => json!({ "labels": names }),
    };
    let answer = gitlab(
        ctx,
        "PUT",
        &format!("{}/issues/{number}", repo.path),
        Call::new(format!("{action} labels on #{number} of {}", repo.full)).json(json),
    )
    .await?;

    let body = expect_body(&answer.body, "web_url", "an issue")?;
    let now = names_in(at(body, "labels"), "name");
    let has = |name: &str| now.iter().any(|entry| entry.to_lowercase() == name.to_lowercase());
    let not_applied: Vec<String> = if action == "remove" {
        labels.iter().filter(|l| has(l)).cloned().collect()
    } else {
        labels.iter().filter(|l| !has(l)).cloned().collect()
    };

    let current = if now.is_empty() { "none".to_string() } else { now.join(", ") };
    let note = if not_applied.is_empty() {
        String::new()
    } else {
        format!(" {}: {}.", if action == "remove" { "GitLab kept" } else { "GitLab did not apply" }, not_applied.join(", "))
    };
    Ok(Outcome::ok(
        format!("Labels on #{number} in {}: {current}.{note}", repo.full),
        Fields::new()
            .set("number", number)
            .set("labels", now)
            .opt("not_applied", (!not_applied.is_empty()).then_some(not_applied)),
    ))
}
