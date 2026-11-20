//! People, by username. GitLab assigns by numeric user id, and the tools take the username a person would
//! write, so a name is looked up before anything is changed.
use serde_json::Value;

use super::api::{Call, gitlab};
use super::context::Ctx;
use crate::utils::errors::Refused;
use crate::utils::js::{as_int, list};

/// A user GitLab knows, by id and username.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundUser {
    pub id: i64,
    pub username: String,
}

/// A username as people write it, with or without the `@`.
pub fn bare_username(name: &str) -> String {
    name.strip_prefix('@').unwrap_or(name).to_string()
}

/// Find the users behind some usernames, and refuse before any write if one does not exist.
///
/// A name GitLab does not know is a typo far more often than a wish, and sending the rest anyway would
/// assign half of what was asked and say nothing about it. `names` are already trimmed and without
/// duplicates; `what` completes "... to <what>" in a refusal. The answer is in the order asked.
pub async fn find_users(
    ctx: &Ctx,
    names: &[String],
    what: &str,
) -> Result<Vec<FoundUser>, Refused> {
    let mut found = Vec::new();
    let mut unknown = Vec::new();

    for requested in names {
        let username = bare_username(requested);
        let answer = gitlab(
            ctx,
            "GET",
            "/users",
            Call::new(format!("look up the user '{username}' to {what}"))
                .query("username", Some(&username)),
        )
        .await?;
        let user = list(&answer.body).iter().find(|candidate| {
            candidate
                .get("username")
                .and_then(Value::as_str)
                .is_some_and(|name| name.to_lowercase() == username.to_lowercase())
        });
        match user.and_then(|user| as_int(&user["id"]).map(|id| (id, user))) {
            Some((id, user)) => found.push(FoundUser {
                id,
                username: user["username"].as_str().unwrap_or_default().to_string(),
            }),
            None => unknown.push(username),
        }
    }

    if !unknown.is_empty() {
        let names = unknown
            .iter()
            .map(|name| format!("'{name}'"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Refused(format!(
            "GitLab has no user named {names}, so nothing was changed. Write the username as it appears after \
             the @ on the profile page."
        )));
    }
    Ok(found)
}
