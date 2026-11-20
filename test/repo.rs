//! repo_create, repo_read, repo_update, repo_delete, repo_fork and repo_search.
use lxagents_gitlab::config::config::TOKEN_ENV;
use lxagents_gitlab::config::version::VERSION;
use lxagents_gitlab::controllers::get;
use lxagents_gitlab::services::context::TransportError;
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;

/* ------------------------------------------------------------------ repo_create */

fn project(overrides: Value) -> Value {
    with(
        json!({
            "id": 7, "name": "demo", "path": "demo", "path_with_namespace": "me/demo", "web_url": "https://gitlab.com/me/demo",
            "http_url_to_repo": "https://gitlab.com/me/demo.git", "visibility": "private", "default_branch": "main",
        }),
        overrides,
    )
}

fn route(t: &Harness) -> Vec<String> {
    t.calls()
        .iter()
        .map(|c| format!("{} {}", c.method, c.url.replace(API, "")))
        .collect()
}

#[test]
fn repo_create_the_token_variable_is_the_one_the_owner_named() {
    assert_eq!(TOKEN_ENV, "GITLAB_API_KEY");
    let def = &get("repo_create").unwrap().def;
    assert_eq!(def.name, "repo_create");
    assert!(def.description.contains(TOKEN_ENV));
    let keys: Vec<&str> = def.input_schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "name",
            "group",
            "description",
            "visibility",
            "initialize_with_readme"
        ]
    );
}

#[tokio::test]
async fn repo_create_a_project_is_created_in_the_token_owners_namespace_private_and_initialized_by_default()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(201, project(json!({})))]);
    let outcome = t.outcome("repo_create", json!({ "name": "demo" })).await;
    assert!(outcome.ok);
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Created private repository me/demo on GitLab.", "name": "demo", "full_name": "me/demo",
            "html_url": "https://gitlab.com/me/demo", "clone_url": "https://gitlab.com/me/demo.git", "visibility": "private",
            "default_branch": "main",
        })
    );
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, format!("{API}/projects"));
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].body(),
        &json!({ "name": "demo", "path": "demo", "visibility": "private", "initialize_with_readme": true })
    );
    assert_eq!(
        calls[0].header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(
        calls[0].header("User-Agent"),
        Some(format!("lxagents-mcp-gitlab/{VERSION}").as_str())
    );
    assert_eq!(calls[0].header("Content-Type"), Some("application/json"));
}

#[tokio::test]
async fn repo_create_a_group_is_looked_up_by_its_encoded_path_and_its_id_becomes_the_namespace() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "id": 42, "full_path": "acme/tools" })),
        reply(
            201,
            project(json!({ "path_with_namespace": "acme/tools/demo", "visibility": "internal" })),
        ),
    ]);
    let payload = t
        .ok(
            "repo_create",
            json!({ "name": "demo", "group": "acme/tools", "description": "  A demo  ", "visibility": "internal", "initialize_with_readme": false }),
        )
        .await;
    assert_eq!(
        payload["summary"],
        json!("Created internal repository acme/tools/demo on GitLab.")
    );
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, format!("{API}/groups/acme%2Ftools"));
    assert_eq!(calls[0].method, "GET");
    assert_eq!(
        calls[0].header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(
        calls[1].body(),
        &json!({ "name": "demo", "path": "demo", "visibility": "internal", "initialize_with_readme": false, "namespace_id": 42, "description": "A demo" })
    );
}

#[tokio::test]
async fn repo_create_a_blank_group_means_the_personal_namespace_and_no_lookup() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, project(json!({})))]);
    t.ok("repo_create", json!({ "name": "demo", "group": "   " }))
        .await;
    assert_eq!(t.call_count(), 1);
    assert_eq!(t.calls()[0].url, format!("{API}/projects"));
}

#[tokio::test]
async fn repo_create_a_missing_default_branch_is_left_out_and_visibility_falls_back_to_what_was_asked()
 {
    let t = Harness::with_token();
    let mut answer = project(json!({ "default_branch": null }));
    answer.as_object_mut().unwrap().remove("visibility");
    t.mock(vec![reply(201, answer)]);
    let payload = t
        .ok(
            "repo_create",
            json!({ "name": "demo", "visibility": "public" }),
        )
        .await;
    assert_eq!(payload["visibility"], json!("public"));
    assert!(payload.get("default_branch").is_none());
}

#[tokio::test]
async fn repo_create_a_missing_token_is_a_refusal_that_names_the_variable_and_nothing_is_sent() {
    let t = Harness::new();
    t.mock(vec![reply(201, project(json!({})))]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        r"^GITLAB_API_KEY is not set\.",
    );
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn repo_create_bad_arguments_are_refused_before_the_token_is_read_or_anything_is_sent() {
    let t = Harness::new();
    t.mock(vec![reply(201, project(json!({})))]);
    let cases: Vec<(Value, &str)> = vec![
        (json!({}), "`name` is required"),
        (json!({ "name": "   " }), "`name` is required"),
        (json!({ "name": 5 }), "`name` must be a string"),
        (
            json!({ "name": "has space" }),
            "not a valid GitLab repository name",
        ),
        (
            json!({ "name": "a/b" }),
            "not a valid GitLab repository name",
        ),
        (
            json!({ "name": "-lead" }),
            "not a valid GitLab repository name",
        ),
        (
            json!({ "name": ".lead" }),
            "not a valid GitLab repository name",
        ),
        (
            json!({ "name": "repo.git" }),
            "not a valid GitLab repository name",
        ),
        (
            json!({ "name": "repo.ATOM" }),
            "not a valid GitLab repository name",
        ),
        (
            json!({ "name": "ok", "group": "a//b" }),
            "not a valid GitLab group path",
        ),
        (
            json!({ "name": "ok", "group": "/a" }),
            "not a valid GitLab group path",
        ),
        (
            json!({ "name": "ok", "group": vec!["g"; 21].join("/") }),
            "not a valid GitLab group path",
        ),
        (
            json!({ "name": "ok", "visibility": "secret" }),
            "`visibility` must be one of private, internal, public, got 'secret'",
        ),
        (
            json!({ "name": "ok", "initialize_with_readme": "yes" }),
            "`initialize_with_readme` must be true or false",
        ),
        (
            json!({ "name": "ok", "description": 3 }),
            "`description` must be a string",
        ),
    ];
    for (args, pattern) in cases {
        assert_match(&t.refused("repo_create", args).await, pattern);
    }
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn repo_create_401_says_the_token_was_rejected_and_stops_before_any_project_is_created() {
    let t = Harness::with_token();
    t.mock(vec![reply(401, json!({ "message": "401 Unauthorized" }))]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo", "group": "acme" }))
            .await,
        r"^GitLab rejected the token \(401\)\. Check that GITLAB_API_KEY holds a valid token",
    );
    assert_eq!(t.call_count(), 1);
}

#[tokio::test]
async fn repo_create_404_on_the_group_says_the_group_may_not_exist_and_no_project_is_created() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        404,
        json!({ "message": "404 Group Not Found" }),
    )]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo", "group": "ghost" }))
            .await,
        r"could not find the group 'ghost' \(404\).*omit `group`",
    );
    assert_eq!(t.call_count(), 1);
}

#[tokio::test]
async fn repo_create_a_group_answer_without_an_id_is_refused() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({ "unexpected": true }))]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo", "group": "acme" }))
            .await,
        "answered 200 for the group 'acme', but without an id",
    );
}

#[tokio::test]
async fn repo_create_403_says_what_the_token_needs_for_the_namespace_or_the_group() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(403, json!({ "message": "403 Forbidden" })),
        reply(403, json!({ "message": "403 Forbidden" })),
    ]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        r"403\): 403 Forbidden\. The token needs the `api` scope.* in the token owner's namespace\.$",
    );
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo", "group": "acme" }))
            .await,
        r"GitLab refused the request \(403\)",
    );
}

#[tokio::test]
async fn repo_create_403_on_the_create_step_names_the_group() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "id": 1 })),
        reply(403, json!({ "message": "403 Forbidden" })),
    ]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo", "group": "acme" }))
            .await,
        "create projects in 'acme'",
    );
}

#[tokio::test]
async fn repo_create_400_carries_gitlabs_field_errors_such_as_a_name_that_is_taken() {
    let t = Harness::with_token();
    t.mock(vec![reply(400, json!({ "message": { "name": ["has already been taken"], "path": ["has already been taken"] } }))]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        r"could not create 'demo' \(400\): name has already been taken; path has already been taken\..*already exists in the token owner's namespace",
    );
}

#[tokio::test]
async fn repo_create_an_oauth_style_error_is_reported_with_its_description() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        403,
        json!({ "error": "insufficient_scope", "error_description": "needs api" }),
    )]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        r"\(403\): insufficient_scope \(needs api\)\.",
    );
}

#[tokio::test]
async fn repo_create_any_other_status_is_reported_with_its_status_and_message() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        500,
        json!({ "message": "500 Internal Server Error" }),
    )]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        r"^GitLab answered 500: 500 Internal Server Error\.$",
    );
}

#[tokio::test]
async fn repo_create_a_failure_never_carries_the_token_even_when_gitlab_echoes_it() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        500,
        json!({ "message": format!("bad token {TOKEN}") }),
    )]);
    let message = t.refused("repo_create", json!({ "name": "demo" })).await;
    assert!(
        !message.contains(TOKEN) && message.contains("***"),
        "{message}"
    );
}

#[tokio::test]
async fn repo_create_a_201_whose_body_is_not_a_project_is_refused() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, json!({ "unexpected": true }))]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        "answered 201, but the body was not a project",
    );
}

#[tokio::test]
async fn repo_create_a_network_failure_is_a_refusal() {
    let t = Harness::with_token();
    t.mock(vec![failure(TransportError::Failed("ECONNREFUSED".into()))]);
    assert_match(
        &t.refused("repo_create", json!({ "name": "demo" })).await,
        r"^Could not reach gitlab\.com: ECONNREFUSED\.",
    );
}

/* ------------------------------------------------------------------ the other project tools */

/// A project as GitLab returns it to a token.
fn full(overrides: Value) -> Value {
    with(
        json!({
            "id": 7, "name": "r", "path": "r", "path_with_namespace": "g/r", "description": "A thing",
            "web_url": "https://gitlab.com/g/r", "http_url_to_repo": "https://gitlab.com/g/r.git", "visibility": "public",
            "default_branch": "main", "archived": false, "star_count": 5, "forks_count": 2, "open_issues_count": 3,
            "license": { "key": "mit", "name": "MIT License" }, "topics": ["mcp"], "last_activity_at": "2026-10-01T00:00:00.000Z",
        }),
        overrides,
    )
}

fn without(mut value: Value, keys: &[&str]) -> Value {
    for key in keys {
        value.as_object_mut().unwrap().remove(*key);
    }
    value
}

#[tokio::test]
async fn repo_read_reads_one_project_asks_for_its_license_and_reports_its_facts() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, full(json!({})))]);
    let outcome = t.outcome("repo_read", json!({ "repo": "g/r" })).await;
    assert!(outcome.ok);
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "g/r is a public project, default branch main.", "id": 7, "full_name": "g/r", "description": "A thing",
            "html_url": "https://gitlab.com/g/r", "clone_url": "https://gitlab.com/g/r.git", "visibility": "public",
            "default_branch": "main", "fork": false, "archived": false, "stars": 5, "forks": 2, "open_issues": 3,
            "license": "MIT License", "topics": ["mcp"], "last_activity_at": "2026-10-01T00:00:00.000Z",
        })
    );
    assert_eq!(t.call_count(), 1);
    assert_eq!(t.calls()[0].method, "GET");
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr?license=true")
    );
}

#[tokio::test]
async fn repo_read_takes_a_numeric_id_and_a_nested_groups_path_as_one_encoded_segment() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, full(json!({}))),
        reply(200, full(json!({ "path_with_namespace": "a/b/c" }))),
    ]);
    t.ok("repo_read", json!({ "repo": "278964" })).await;
    t.ok("repo_read", json!({ "repo": "a/b/c" })).await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/278964?license=true")
    );
    assert_eq!(
        t.calls()[1].url,
        format!("{API}/projects/a%2Fb%2Fc?license=true")
    );
}

#[tokio::test]
async fn repo_read_says_when_a_project_is_an_archived_fork_scheduled_for_deletion_and_leaves_out_what_gitlab_did_not_give()
 {
    let t = Harness::with_token();
    let answer = without(
        full(json!({
            "archived": true, "description": null, "license": null, "tag_list": ["old"],
            "forked_from_project": { "path_with_namespace": "up/stream" }, "marked_for_deletion_on": "2026-11-09",
        })),
        &["topics", "last_activity_at", "default_branch"],
    );
    t.mock(vec![reply(200, answer)]);
    let payload = t.ok("repo_read", json!({ "repo": "g/r" })).await;
    assert_eq!(
        payload["summary"],
        json!("g/r is a public archived fork, scheduled for deletion on 2026-11-09.")
    );
    assert_eq!(payload["parent"], json!("up/stream"));
    assert_eq!(payload["scheduled_for_deletion"], json!("2026-11-09"));
    assert_eq!(payload["topics"], json!(["old"]));
    for absent in [
        "description",
        "license",
        "last_activity_at",
        "default_branch",
    ] {
        assert!(payload.get(absent).is_none(), "{absent}");
    }
}

#[tokio::test]
async fn repo_read_reads_what_an_anonymous_answer_leaves_out_as_nothing() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        without(
            full(json!({})),
            &["archived", "open_issues_count", "star_count", "forks_count"],
        ),
    )]);
    let payload = t.ok("repo_read", json!({ "repo": "g/r" })).await;
    assert_eq!(payload["archived"], json!(false));
    assert_eq!(
        [
            &payload["stars"],
            &payload["forks"],
            &payload["open_issues"]
        ],
        [&json!(0), &json!(0), &json!(0)]
    );
}

#[tokio::test]
async fn repo_read_refuses_a_bad_repo_before_any_request_a_404_with_the_cause_and_an_unexpected_body()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Project Not Found" })),
        reply(200, json!({ "nope": true })),
    ]);
    assert_match(
        &t.refused("repo_read", json!({ "repo": "nope" })).await,
        "`repo` 'nope' is not a project",
    );
    assert_match(
        &t.refused("repo_read", json!({})).await,
        "`repo` is required",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("repo_read", json!({ "repo": "g/r" })).await,
        r"^GitLab could not find what it needs to read the project g/r \(404\): 404 Project Not Found\. .*private project",
    );
    assert_match(
        &t.refused("repo_read", json!({ "repo": "g/r" })).await,
        "the body was not a project this server recognizes",
    );
}

#[tokio::test]
async fn repo_update_sends_only_what_changes_and_says_what_it_changed() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        full(json!({ "description": "New", "visibility": "private" })),
    )]);
    let payload = t.ok("repo_update", json!({ "repo": "g/r", "description": "New", "visibility": "private", "has_issues": false })).await;
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(calls[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(
        calls[0].body(),
        &json!({ "description": "New", "visibility": "private", "issues_access_level": "disabled" })
    );
    assert_eq!(
        payload["summary"],
        json!(
            "Updated g/r: description, visibility, has_issues. g/r is a private project, default branch main."
        )
    );
}

#[tokio::test]
async fn repo_update_can_clear_a_description_rename_move_the_path_and_change_the_default_branch() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, full(json!({ "path_with_namespace": "g/renamed", "description": null, "default_branch": "trunk" })))]);
    let payload = t
        .ok(
            "repo_update",
            json!({ "repo": "g/r", "description": "", "name": " Renamed ", "path": "renamed", "default_branch": "trunk", "has_issues": true }),
        )
        .await;
    assert_eq!(
        t.calls()[0].body(),
        &json!({ "name": "Renamed", "path": "renamed", "description": "", "default_branch": "trunk", "issues_access_level": "enabled" })
    );
    assert_eq!(payload["full_name"], json!("g/renamed"));
    assert!(payload.get("description").is_none());
}

#[tokio::test]
async fn repo_update_archives_with_its_own_endpoint_after_the_settings_and_addresses_it_by_id() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            full(json!({ "path_with_namespace": "g/renamed", "description": "New" })),
        ),
        reply(
            200,
            full(json!({ "path_with_namespace": "g/renamed", "archived": true })),
        ),
    ]);
    let payload = t
        .ok(
            "repo_update",
            json!({ "repo": "g/r", "path": "renamed", "description": "New", "archived": true }),
        )
        .await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(calls[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(calls[1].url, format!("{API}/projects/7/archive"));
    assert!(calls[1].body.is_none());
    assert_eq!(payload["archived"], json!(true));
    assert_eq!(payload["full_name"], json!("g/renamed"));
}

#[tokio::test]
async fn repo_update_unarchives_before_it_changes_anything_else_and_archiving_alone_is_one_call() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, full(json!({}))),
        reply(200, full(json!({ "description": "New" }))),
        reply(200, full(json!({ "archived": true }))),
    ]);
    t.ok(
        "repo_update",
        json!({ "repo": "g/r", "archived": false, "description": "New" }),
    )
    .await;
    assert_eq!(
        route(&t),
        ["POST /projects/g%2Fr/unarchive", "PUT /projects/7"]
    );
    t.ok("repo_update", json!({ "repo": "g/r", "archived": true }))
        .await;
    assert_eq!(t.call_count(), 3);
    assert_eq!(route(&t)[2], "POST /projects/g%2Fr/archive");
}

#[tokio::test]
async fn repo_update_says_what_had_already_happened_when_a_later_step_is_refused() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, full(json!({}))),
        reply(
            400,
            json!({ "message": { "default_branch": ["does not exist"] } }),
        ),
        reply(200, full(json!({}))),
        reply(403, json!({ "message": "403 Forbidden" })),
    ]);
    assert_match(
        &t.refused(
            "repo_update",
            json!({ "repo": "g/r", "archived": false, "default_branch": "nope" }),
        )
        .await,
        r"default_branch does not exist\. The project had already been unarchived\.$",
    );
    assert_match(
        &t.refused(
            "repo_update",
            json!({ "repo": "g/r", "description": "x", "archived": true }),
        )
        .await,
        r"^GitLab refused to archive the project g/r \(403\).*The other settings had already been changed\.$",
    );
}

#[tokio::test]
async fn repo_update_refuses_an_empty_change_and_bad_values_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, full(json!({})))]);
    let cases: Vec<(Value, &str)> = vec![
        (
            json!({ "repo": "g/r" }),
            "^Nothing to change: pass at least one of name, path,",
        ),
        (
            json!({ "repo": "g/r", "path": "has space" }),
            "not a valid GitLab project path",
        ),
        (
            json!({ "repo": "g/r", "path": "x.git" }),
            "not a valid GitLab project path",
        ),
        (
            json!({ "repo": "g/r", "visibility": "secret" }),
            "`visibility` must be one of private, internal, public, got 'secret'",
        ),
        (
            json!({ "repo": "g/r", "default_branch": "a..b" }),
            "`default_branch` 'a..b' is not a valid",
        ),
        (
            json!({ "repo": "g/r", "archived": 1 }),
            "`archived` must be true or false",
        ),
        (
            json!({ "repo": "g/r", "has_issues": "yes" }),
            "`has_issues` must be true or false",
        ),
        (
            json!({ "repo": "g/r", "description": 5 }),
            "`description` must be a string",
        ),
    ];
    for (args, pattern) in cases {
        assert_match(&t.refused("repo_update", args).await, pattern);
    }
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn repo_update_carries_gitlabs_reason_when_it_cannot_change_a_setting() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        400,
        json!({ "message": { "visibility_level": ["private is not allowed since the fork source project has public visibility."] } }),
    )]);
    assert_match(
        &t.refused(
            "repo_update",
            json!({ "repo": "g/r", "visibility": "private" }),
        )
        .await,
        r"^GitLab could not update the project g/r \(400\): visibility_level private is not allowed since the fork source project has public visibility\.\.$",
    );
}

#[tokio::test]
async fn repo_delete_deletes_only_when_confirm_repeats_the_full_path_ignoring_case() {
    let t = Harness::with_token();
    t.mock(vec![reply(202, json!({ "message": "202 Accepted" }))]);
    let payload = t
        .ok("repo_delete", json!({ "repo": "g/r", "confirm": " G/R " }))
        .await;
    assert_eq!(t.call_count(), 1);
    assert_eq!(t.calls()[0].method, "DELETE");
    assert_eq!(t.calls()[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(payload["accepted"], json!(true));
    assert_eq!(payload["full_name"], json!("g/r"));
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"^GitLab accepted the deletion of g/r\. It may mark the project for deletion and remove it after a retention period",
    );
}

#[tokio::test]
async fn repo_delete_resolves_a_numeric_id_to_its_path_before_it_compares() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, full(json!({}))),
        reply(202, json!({ "message": "202 Accepted" })),
        reply(200, full(json!({}))),
    ]);
    let payload = t
        .ok("repo_delete", json!({ "repo": "7", "confirm": "g/r" }))
        .await;
    assert_eq!(route(&t), ["GET /projects/7", "DELETE /projects/7"]);
    assert_eq!(payload["full_name"], json!("g/r"));
    assert_match(
        &t.refused("repo_delete", json!({ "repo": "7", "confirm": "7" }))
            .await,
        "must repeat the project's full path, 'g/r'",
    );
    assert_eq!(
        t.call_count(),
        3,
        "only the read was sent for the refused call"
    );
}

#[tokio::test]
async fn repo_delete_refuses_without_a_matching_confirm_and_sends_nothing() {
    let t = Harness::with_token();
    t.mock(vec![reply(202, json!({}))]);
    assert_match(
        &t.refused(
            "repo_delete",
            json!({ "repo": "g/r", "confirm": "g/other" }),
        )
        .await,
        r"^`confirm` must repeat the project's full path, 'g/r', to delete it\. Nothing was deleted\.$",
    );
    assert_match(
        &t.refused("repo_delete", json!({ "repo": "g/r", "confirm": "r" }))
            .await,
        "Nothing was deleted",
    );
    assert_match(
        &t.refused("repo_delete", json!({ "repo": "g/r" })).await,
        "`confirm` is required",
    );
    assert_match(
        &t.refused("repo_delete", json!({ "repo": "g/r", "confirm": true }))
            .await,
        "`confirm` must be a string",
    );
    assert_match(
        &t.refused("repo_delete", json!({ "confirm": "g/r" })).await,
        "`repo` is required",
    );
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn repo_delete_says_what_deleting_needs_and_that_a_missing_project_is_a_404() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(403, json!({ "message": "403 Forbidden" })),
        reply(404, json!({ "message": "404 Project Not Found" })),
    ]);
    assert_match(
        &t.refused("repo_delete", json!({ "repo": "g/r", "confirm": "g/r" }))
            .await,
        r"^GitLab refused to delete g/r \(403\): 403 Forbidden\. Deleting needs the Owner role on the project",
    );
    assert_match(
        &t.refused("repo_delete", json!({ "repo": "g/r", "confirm": "g/r" }))
            .await,
        r"could not find what it needs to delete the project g/r \(404\)",
    );
}

#[tokio::test]
async fn repo_fork_forks_into_the_token_owners_namespace_by_default_and_says_gitlab_finishes_it_later()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(
        201,
        full(json!({ "path_with_namespace": "me/r", "forked_from_project": { "path_with_namespace": "g/r" }, "import_status": "scheduled" })),
    )]);
    let payload = t.ok("repo_fork", json!({ "repo": "g/r" })).await;
    assert_eq!(t.calls()[0].method, "POST");
    assert_eq!(t.calls()[0].url, format!("{API}/projects/g%2Fr/fork"));
    assert_eq!(t.calls()[0].body(), &json!({}));
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"^Forking g/r into me/r has started\. GitLab finishes the copy in the background",
    );
    assert_eq!(payload["parent"], json!("g/r"));
    assert_eq!(payload["fork"], json!(true));
    assert_eq!(payload["import_status"], json!("scheduled"));
}

#[tokio::test]
async fn repo_fork_passes_the_group_the_new_name_and_the_new_path() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        201,
        full(json!({ "path_with_namespace": "acme/tools/copy", "forked_from_project": {} })),
    )]);
    let payload = t
        .ok(
            "repo_fork",
            json!({ "repo": "g/r", "group": " acme/tools ", "name": "Copy", "path": "copy" }),
        )
        .await;
    assert_eq!(
        t.calls()[0].body(),
        &json!({ "namespace_path": "acme/tools", "name": "Copy", "path": "copy" })
    );
    assert!(payload.get("import_status").is_none());
}

#[tokio::test]
async fn repo_fork_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, full(json!({})))]);
    assert_match(
        &t.refused("repo_fork", json!({ "repo": "nope" })).await,
        "is not a project",
    );
    assert_match(
        &t.refused("repo_fork", json!({ "repo": "g/r", "group": "a//b" }))
            .await,
        "`group` 'a//b' is not a valid GitLab namespace path",
    );
    assert_match(
        &t.refused("repo_fork", json!({ "repo": "g/r", "path": "a b" }))
            .await,
        "`path` 'a b' is not a valid GitLab project path",
    );
    assert_match(
        &t.refused("repo_fork", json!({ "repo": "g/r", "name": 5 }))
            .await,
        "`name` must be a string",
    );
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn repo_search_sends_the_query_and_the_paging_and_reports_each_match() {
    let t = Harness::with_token();
    t.mock(vec![reply_with(
        200,
        json!([
            { "path_with_namespace": "a/mcp", "description": "First", "star_count": 9, "last_activity_at": "2026-10-01T00:00:00Z", "web_url": "https://gitlab.com/a/mcp" },
            { "path_with_namespace": "b/mcp", "description": null, "star_count": 1, "web_url": "https://gitlab.com/b/mcp" },
        ]),
        &[("x-total", "37"), ("x-next-page", "2")],
    )]);
    let payload = t
        .ok(
            "repo_search",
            json!({ "query": " mcp server ", "sort": "star_count", "order": "asc", "per_page": 2 }),
        )
        .await;
    let call = &t.calls()[0];
    let url = call.parsed();
    assert_eq!(
        format!("{}{}", url.origin().ascii_serialization(), url.path()),
        format!("{API}/projects")
    );
    let expected: std::collections::BTreeMap<String, String> = [
        ("search", "mcp server"),
        ("order_by", "star_count"),
        ("sort", "asc"),
        ("page", "1"),
        ("per_page", "2"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    assert_eq!(call.query(), expected);
    assert_eq!(
        payload,
        json!({
            "summary": "37 projects match 'mcp server'; showing 2.",
            "items": [
                { "full_name": "a/mcp", "description": "First", "stars": 9, "last_activity_at": "2026-10-01T00:00:00Z", "url": "https://gitlab.com/a/mcp" },
                { "full_name": "b/mcp", "stars": 1, "url": "https://gitlab.com/b/mcp" },
            ],
            "total_count": 37, "page": 1, "per_page": 2, "has_more": true,
        })
    );
}

#[tokio::test]
async fn repo_search_leaves_the_direction_out_unless_there_is_a_sort_falls_back_to_the_page_and_reports_no_match_plainly()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!([{ "path_with_namespace": "a/x", "star_count": 0, "web_url": "https://gitlab.com/a/x" }])), reply(200, json!([]))]);
    let some = t
        .ok("repo_search", json!({ "query": "x", "order": "asc" }))
        .await;
    let query = t.calls()[0].query();
    assert!(!query.contains_key("order_by"));
    assert!(!query.contains_key("sort"));
    assert_eq!(some["total_count"], json!(1));
    assert_eq!(some["summary"], json!("1 project matches 'x'; showing 1."));
    assert_eq!(some["has_more"], json!(false));
    let none = t.ok("repo_search", json!({ "query": "zzzz" })).await;
    assert_eq!(none["summary"], json!("No projects match 'zzzz'."));
    assert_eq!(none["total_count"], json!(0));
}

#[tokio::test]
async fn repo_search_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!([]))]);
    assert_match(
        &t.refused("repo_search", json!({})).await,
        "`query` is required",
    );
    assert_match(
        &t.refused("repo_search", json!({ "query": "  " })).await,
        "`query` is required",
    );
    assert_match(
        &t.refused("repo_search", json!({ "query": "x", "sort": "best" }))
            .await,
        "`sort` must be one of name, path, created_at,.*got 'best'",
    );
    assert_match(
        &t.refused("repo_search", json!({ "query": "x", "order": "up" }))
            .await,
        "`order` must be one of asc, desc",
    );
    assert_match(
        &t.refused("repo_search", json!({ "query": "x", "per_page": 101 }))
            .await,
        "`per_page` must be between 1 and 100",
    );
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn repo_search_passes_on_gitlabs_reason_for_a_query_it_cannot_run_and_its_rate_limit() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(400, json!({ "message": "search is too short" })),
        reply_with(
            429,
            json!({ "message": "Retry later" }),
            &[("retry-after", "20")],
        ),
    ]);
    assert_match(
        &t.refused("repo_search", json!({ "query": "x" })).await,
        r"^GitLab could not search projects for 'x' \(400\): search is too short\.$",
    );
    assert_match(
        &t.refused("repo_search", json!({ "query": "x" })).await,
        r"rate limiting this token \(429\): Retry later\. Wait 20 seconds",
    );
}
