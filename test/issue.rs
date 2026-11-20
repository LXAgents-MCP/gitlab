//! issue_open, issue_read, issue_comment, issue_close, issue_assign, issue_label and issue_lock.
use lxagents_gitlab::models::args::{ListOptions, label_list};
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;

fn base() -> String {
    format!("{API}/projects/g%2Fr/issues/7")
}

pub fn user(id: i64, username: &str) -> Value {
    json!({ "id": id, "username": username, "name": username, "state": "active" })
}

/// An issue as GitLab returns it.
fn issue(overrides: Value) -> Value {
    with(
        json!({
            "id": 9001, "iid": 7, "project_id": 3, "title": "It crashes", "description": "Steps:\n\n1. run it",
            "state": "opened", "created_at": "2026-10-01T10:00:00.000Z", "updated_at": "2026-10-02T10:00:00.000Z",
            "closed_at": null, "labels": ["bug", "p1"], "milestone": { "id": 4, "iid": 1, "title": "v1" },
            "assignees": [user(11, "alice")], "author": user(12, "bob"), "user_notes_count": 2,
            "confidential": false, "discussion_locked": null, "web_url": "https://gitlab.com/g/r/-/issues/7",
        }),
        overrides,
    )
}

pub fn note(id: i64, body: &str, overrides: Value) -> Value {
    with(
        json!({ "id": id, "body": body, "author": user(12, "bob"), "created_at": "2026-10-01T11:00:00.000Z", "system": false }),
        overrides,
    )
}

/* ------------------------------------------------------------------ issue_open */

#[tokio::test]
async fn issue_open_opens_an_issue_with_labels_assignees_and_a_milestone_and_reports_what_gitlab_applied()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!([user(11, "alice")])),
        reply(201, issue(json!({}))),
    ]);
    let outcome = t
        .outcome(
            "issue_open",
            json!({ "repo": "g/r", "title": " It crashes ", "body": "Steps:\n\n1. run it", "labels": ["bug", "p1"], "assignees": ["@alice"], "milestone": 4 }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, format!("{API}/users?username=alice"));
    assert_eq!(calls[0].method, "GET");
    assert_eq!(calls[1].url, format!("{API}/projects/g%2Fr/issues"));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(
        calls[1].body(),
        &json!({ "title": "It crashes", "description": "Steps:\n\n1. run it", "labels": "bug,p1", "assignee_ids": [11], "milestone_id": 4 })
    );
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Opened issue #7 in g/r: It crashes.", "number": 7, "title": "It crashes", "state": "opened",
            "html_url": "https://gitlab.com/g/r/-/issues/7", "labels": ["bug", "p1"], "assignees": ["alice"],
        })
    );
}

#[tokio::test]
async fn issue_open_sends_only_the_title_when_that_is_all_it_was_given_and_looks_nobody_up() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        201,
        issue(json!({ "labels": [], "assignees": [], "milestone": null })),
    )]);
    let payload = t
        .ok(
            "issue_open",
            json!({ "repo": "g/r", "title": "Plain", "labels": [], "assignees": [] }),
        )
        .await;
    assert_eq!(t.call_count(), 1);
    assert_eq!(t.calls()[0].body(), &json!({ "title": "Plain" }));
    assert_eq!(payload["labels"], json!([]));
    assert_eq!(payload["assignees"], json!([]));
}

#[tokio::test]
async fn issue_open_says_what_gitlab_did_not_apply() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!([user(11, "alice")])),
        reply(200, json!([user(13, "carol")])),
        reply(
            201,
            issue(json!({ "assignees": [user(11, "alice")], "labels": ["bug"] })),
        ),
    ]);
    let payload = t.ok("issue_open", json!({ "repo": "g/r", "title": "x", "labels": ["Bug", "p1"], "assignees": ["alice", "carol"] })).await;
    let summary = payload["summary"].as_str().unwrap();
    assert_match(
        summary,
        r"^Opened issue #7 in g/r: It crashes\. GitLab did not apply: label p1, assignee carol\.",
    );
    assert_match(summary, "A free plan keeps one assignee");
}

#[tokio::test]
async fn issue_open_refuses_an_unknown_username_before_it_writes_anything() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!([user(11, "alice")])),
        reply(200, json!([])),
    ]);
    assert_match(
        &t.refused(
            "issue_open",
            json!({ "repo": "g/r", "title": "x", "assignees": ["alice", "nobody"] }),
        )
        .await,
        "GitLab has no user named 'nobody', so nothing was changed",
    );
    assert_eq!(t.call_count(), 2);
    assert!(t.calls().iter().all(|c| c.method == "GET"));
}

#[tokio::test]
async fn issue_open_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused("issue_open", json!({ "repo": "g/r" })).await,
        "`title` is required",
    );
    assert_match(
        &t.refused(
            "issue_open",
            json!({ "repo": "g/r", "title": "x", "body": 3 }),
        )
        .await,
        "`body` must be a string",
    );
    assert_match(
        &t.refused(
            "issue_open",
            json!({ "repo": "g/r", "title": "x", "labels": ["a,b"] }),
        )
        .await,
        "`labels` may not contain a comma.*'a,b'",
    );
    assert_match(
        &t.refused(
            "issue_open",
            json!({ "repo": "g/r", "title": "x", "labels": "bug" }),
        )
        .await,
        "`labels` must be a list of strings",
    );
    let many: Vec<String> = (0..11).map(|i| format!("u{i}")).collect();
    assert_match(
        &t.refused(
            "issue_open",
            json!({ "repo": "g/r", "title": "x", "assignees": many }),
        )
        .await,
        "at most 10",
    );
    assert_match(
        &t.refused(
            "issue_open",
            json!({ "repo": "g/r", "title": "x", "milestone": 0 }),
        )
        .await,
        "`milestone` must be between 1",
    );
    assert_match(
        &t.refused("issue_open", json!({ "repo": "bad", "title": "x" }))
            .await,
        "is not a project",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("issue_open", json!({ "repo": "g/r", "title": "x" }))
            .await,
        "GITLAB_API_KEY is not set",
    );
}

#[test]
fn label_list_refuses_a_comma_and_passes_everything_else_through() {
    let options = ListOptions {
        required: false,
        max: 100,
    };
    assert_eq!(
        label_list(&json!([" a ", "b", "a"]), "labels", options).unwrap(),
        Some(vec!["a".into(), "b".into()])
    );
    assert_eq!(label_list(&Value::Null, "labels", options).unwrap(), None);
    assert_match(
        &label_list(&json!(["ok", "no,pe"]), "labels", options)
            .unwrap_err()
            .0,
        "'no,pe'",
    );
}

/* ------------------------------------------------------------------ issue_read */

#[tokio::test]
async fn issue_read_reports_an_issue() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, issue(json!({})))]);
    let outcome = t
        .outcome("issue_read", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert!(outcome.ok);
    assert_eq!(t.call_count(), 1, "no comments were asked for");
    assert_eq!(t.calls()[0].url, base());
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Issue #7 in g/r is opened: It crashes.", "number": 7, "title": "It crashes", "state": "opened",
            "author": "bob", "labels": ["bug", "p1"], "assignees": ["alice"], "milestone": "v1", "comment_count": 2,
            "locked": false, "confidential": false, "html_url": "https://gitlab.com/g/r/-/issues/7",
            "created_at": "2026-10-01T10:00:00.000Z", "updated_at": "2026-10-02T10:00:00.000Z", "body": "Steps:\n\n1. run it",
        })
    );
}

#[tokio::test]
async fn issue_read_returns_the_comments_only_the_ones_people_wrote_oldest_first() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "user_notes_count": 3, "discussion_locked": true, "state": "closed", "closed_at": "2026-10-03T10:00:00.000Z" }))),
        reply(200, json!([note(1, "first", json!({})), note(2, "added label ~bug", json!({ "system": true })), note(3, "second", json!({ "author": null }))])),
    ]);
    let payload = t
        .ok(
            "issue_read",
            json!({ "repo": "g/r", "number": 7, "comments": true }),
        )
        .await;
    let call = &t.calls()[1];
    assert_eq!(call.path(), "/api/v4/projects/g%2Fr/issues/7/notes");
    assert_eq!(
        call.param("activity_filter").as_deref(),
        Some("only_comments")
    );
    assert_eq!(call.param("sort").as_deref(), Some("asc"));
    assert_eq!(call.param("order_by").as_deref(), Some("created_at"));
    assert_eq!(call.param("per_page").as_deref(), Some("30"));
    assert_eq!(
        payload["comments"],
        json!([
            { "id": 1, "author": "bob", "created_at": "2026-10-01T11:00:00.000Z", "body": "first" },
            { "id": 3, "created_at": "2026-10-01T11:00:00.000Z", "body": "second" },
        ])
    );
    assert_eq!(payload["locked"], true);
    assert_eq!(payload["closed_at"], "2026-10-03T10:00:00.000Z");
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"is closed: It crashes; showing the first 2 comments\.",
    );
}

#[tokio::test]
async fn issue_read_does_not_ask_for_comments_an_issue_does_not_have() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, issue(json!({ "user_notes_count": 0 })))]);
    let payload = t
        .ok(
            "issue_read",
            json!({ "repo": "g/r", "number": 7, "comments": true }),
        )
        .await;
    assert_eq!(t.call_count(), 1);
    assert!(payload.get("comments").is_none());
}

#[tokio::test]
async fn issue_read_refuses_what_is_not_an_issue_and_bad_arguments() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!("<html>")),
        reply(404, json!({ "message": "404 Not found" })),
    ]);
    assert_match(
        &t.refused("issue_read", json!({ "repo": "g/r", "number": 0 }))
            .await,
        "`number` must be between 1",
    );
    assert_match(
        &t.refused("issue_read", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_match(
        &t.refused(
            "issue_read",
            json!({ "repo": "g/r", "number": 7, "comments": "yes" }),
        )
        .await,
        "`comments` must be true or false",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("issue_read", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "body was not an issue this server recognizes",
    );
    assert_match(
        &t.refused("issue_read", json!({ "repo": "g/r", "number": 99 }))
            .await,
        r"could not find what it needs to read issue #99 of g/r \(404\)",
    );
}

/* ------------------------------------------------------------------ issue_comment */

#[tokio::test]
async fn issue_comment_adds_a_comment_and_links_to_it() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, note(55, "Thanks", json!({}))),
        reply(201, note(56, "Thanks", json!({}))),
    ]);
    let outcome = t
        .outcome(
            "issue_comment",
            json!({ "repo": "g/r", "number": 7, "body": "  Thanks\n" }),
        )
        .await;
    assert!(outcome.ok);
    let call = &t.calls()[0];
    assert_eq!(call.url, format!("{}/notes", base()));
    assert_eq!(call.method, "POST");
    assert_eq!(
        call.body(),
        &json!({ "body": "  Thanks\n" }),
        "the text is sent as written"
    );
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Commented on #7 in g/r.", "id": 55, "html_url": "https://gitlab.com/g/r/-/issues/7#note_55" })
    );
    let by_id = t
        .ok(
            "issue_comment",
            json!({ "repo": "3", "number": 7, "body": "x" }),
        )
        .await;
    assert!(
        by_id.get("html_url").is_none(),
        "a project given by id has no path to build a link from"
    );
}

#[tokio::test]
async fn issue_comment_explains_a_refusal_and_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(403, json!({ "message": "403 Forbidden" })),
        reply(201, json!({ "unexpected": true })),
    ]);
    assert_match(
        &t.refused("issue_comment", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "`body` is required",
    );
    assert_match(
        &t.refused(
            "issue_comment",
            json!({ "repo": "g/r", "number": 7, "body": "  " }),
        )
        .await,
        "`body` is required and must not be blank",
    );
    assert_match(
        &t.refused(
            "issue_comment",
            json!({ "repo": "g/r", "number": 1.5, "body": "x" }),
        )
        .await,
        "`number` must be a whole number",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "issue_comment",
            json!({ "repo": "g/r", "number": 7, "body": "x" }),
        )
        .await,
        r"would not let this comment on #7 of g/r \(403\): 403 Forbidden\. .*the discussion may be locked, in which case only project members can comment",
    );
    assert_match(
        &t.refused(
            "issue_comment",
            json!({ "repo": "g/r", "number": 7, "body": "x" }),
        )
        .await,
        "body was not a comment this server recognizes",
    );
}

/* ------------------------------------------------------------------ issue_close */

#[tokio::test]
async fn issue_close_comments_first_then_closes() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, note(1, "Fixed in v2", json!({}))),
        reply(200, issue(json!({ "state": "closed" }))),
    ]);
    let outcome = t
        .outcome(
            "issue_close",
            json!({ "repo": "g/r", "number": 7, "comment": "Fixed in v2" }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, format!("{}/notes", base()));
    assert_eq!(calls[0].body(), &json!({ "body": "Fixed in v2" }));
    assert_eq!(calls[1].url, base());
    assert_eq!(calls[1].method, "PUT");
    assert_eq!(calls[1].body(), &json!({ "state_event": "close" }));
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Closed issue #7 in g/r.", "number": 7, "state": "closed", "html_url": "https://gitlab.com/g/r/-/issues/7" })
    );
}

#[tokio::test]
async fn issue_close_without_a_comment_makes_one_request_and_a_blank_comment_is_none() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "state": "closed" }))),
        reply(200, issue(json!({ "state": "closed" }))),
    ]);
    t.ok("issue_close", json!({ "repo": "g/r", "number": 7 }))
        .await;
    t.ok(
        "issue_close",
        json!({ "repo": "g/r", "number": 7, "comment": "   " }),
    )
    .await;
    assert_eq!(
        t.calls()
            .iter()
            .map(|c| c.method.as_str())
            .collect::<Vec<_>>(),
        ["PUT", "PUT"]
    );
}

#[tokio::test]
async fn issue_close_says_the_comment_was_already_posted_when_closing_fails_and_when_gitlab_left_it_open()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, note(1, "why", json!({}))),
        reply(403, json!({ "message": "403 Forbidden" })),
        reply(200, issue(json!({ "state": "opened" }))),
    ]);
    assert_match(
        &t.refused(
            "issue_close",
            json!({ "repo": "g/r", "number": 7, "comment": "why" }),
        )
        .await,
        r"refused to close issue #7 of g/r \(403\).*The comment had already been posted\.$",
    );
    let payload = t
        .ok("issue_close", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert_eq!(
        payload["summary"],
        "GitLab answered, but issue #7 in g/r is still opened."
    );
}

#[tokio::test]
async fn issue_close_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused("issue_close", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_match(
        &t.refused(
            "issue_close",
            json!({ "repo": "g/r", "number": 7, "comment": 4 }),
        )
        .await,
        "`comment` must be a string",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ issue_assign */

#[tokio::test]
async fn issue_assign_adds_people_to_those_already_assigned() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "assignees": [user(11, "alice")] }))),
        reply(200, json!([user(13, "carol")])),
        reply(
            200,
            issue(json!({ "assignees": [user(11, "alice"), user(13, "carol")] })),
        ),
    ]);
    let outcome = t
        .outcome(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["@carol"] }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, base());
    assert_eq!(calls[1].url, format!("{API}/users?username=carol"));
    assert_eq!(calls[2].method, "PUT");
    assert_eq!(calls[2].body(), &json!({ "assignee_ids": [11, 13] }));
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Assignees of #7 in g/r: alice, carol.", "number": 7, "assignees": ["alice", "carol"] })
    );
}

#[tokio::test]
async fn issue_assign_does_not_write_when_the_people_are_already_assigned() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({}))),
        reply(200, json!([user(11, "Alice")])),
    ]);
    let payload = t
        .ok(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["alice"] }),
        )
        .await;
    assert_eq!(
        t.calls()
            .iter()
            .map(|c| c.method.as_str())
            .collect::<Vec<_>>(),
        ["GET", "GET"]
    );
    assert_eq!(payload["assignees"], json!(["alice"]));
    assert!(payload.get("not_applied").is_none());
}

#[tokio::test]
async fn issue_assign_says_which_people_gitlab_did_not_apply() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "assignees": [] }))),
        reply(200, json!([user(11, "alice")])),
        reply(200, json!([user(13, "carol")])),
        reply(200, issue(json!({ "assignees": [user(11, "alice")] }))),
    ]);
    let payload = t
        .ok(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["alice", "carol"] }),
        )
        .await;
    assert_eq!(payload["not_applied"], json!(["carol"]));
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"^Assignees of #7 in g/r: alice\. GitLab did not assign: carol\. A free plan keeps one assignee per issue\.$",
    );
}

#[tokio::test]
async fn issue_assign_removes_people_and_reports_those_who_were_not_assigned() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            issue(json!({ "assignees": [user(11, "alice"), user(13, "carol")] })),
        ),
        reply(200, issue(json!({ "assignees": [user(13, "carol")] }))),
    ]);
    let payload = t.ok("issue_assign", json!({ "repo": "g/r", "number": 7, "assignees": ["Alice", "dave"], "action": "remove" })).await;
    assert_eq!(t.call_count(), 2, "nobody is looked up to be removed");
    assert_eq!(t.calls()[1].body(), &json!({ "assignee_ids": [13] }));
    assert_eq!(payload["assignees"], json!(["carol"]));
    assert_eq!(payload["not_applied"], json!(["dave"]));
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"Not removed: dave\. They were not assigned, or GitLab kept them\.",
    );
}

#[tokio::test]
async fn issue_assign_unassigns_everyone_with_the_id_0() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "assignees": [user(11, "alice")] }))),
        reply(200, issue(json!({ "assignees": [] }))),
        reply(200, issue(json!({ "assignees": [] }))),
    ]);
    let removed = t
        .ok(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["alice"], "action": "remove" }),
        )
        .await;
    assert_eq!(t.calls()[1].body(), &json!({ "assignee_ids": [0] }));
    assert_eq!(removed["summary"], "Assignees of #7 in g/r: none.");
    let cleared = t
        .ok(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": [], "action": "set" }),
        )
        .await;
    assert_eq!(t.call_count(), 3, "set reads nothing and looks nobody up");
    assert_eq!(t.calls()[2].body(), &json!({ "assignee_ids": [0] }));
    assert_eq!(cleared["assignees"], json!([]));
}

#[tokio::test]
async fn issue_assign_replaces_the_assignees_with_set() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!([user(13, "carol")])),
        reply(200, issue(json!({ "assignees": [user(13, "carol")] }))),
    ]);
    let payload = t
        .ok(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["carol"], "action": "set" }),
        )
        .await;
    assert_eq!(t.call_count(), 2);
    assert_eq!(t.calls()[1].method, "PUT");
    assert_eq!(t.calls()[1].body(), &json!({ "assignee_ids": [13] }));
    assert_eq!(payload["assignees"], json!(["carol"]));
}

#[tokio::test]
async fn issue_assign_refuses_an_unknown_user_before_changing_anything_and_bad_arguments_before_any_request()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(200, issue(json!({}))), reply(200, json!([]))]);
    assert_match(
        &t.refused("issue_assign", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "`assignees` is required",
    );
    assert_match(
        &t.refused(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": [] }),
        )
        .await,
        r"at least one username\. Only action set may be empty",
    );
    assert_match(
        &t.refused(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["a"], "action": "toggle" }),
        )
        .await,
        "`action` must be one of add, remove, set",
    );
    assert_match(
        &t.refused("issue_assign", json!({ "repo": "g/r", "assignees": ["a"] }))
            .await,
        "`number` is required",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "issue_assign",
            json!({ "repo": "g/r", "number": 7, "assignees": ["ghost"] }),
        )
        .await,
        "GitLab has no user named 'ghost', so nothing was changed",
    );
    assert!(t.calls().iter().all(|c| c.method == "GET"));
}

/* ------------------------------------------------------------------ issue_label */

#[tokio::test]
async fn issue_label_adds_removes_and_replaces_labels_in_one_request_each() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "labels": ["bug", "p1", "ui"] }))),
        reply(200, issue(json!({ "labels": ["bug", "ui"] }))),
        reply(200, issue(json!({ "labels": ["wontfix"] }))),
    ]);
    let added = t
        .ok(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["ui", "p1"] }),
        )
        .await;
    assert_eq!(t.calls()[0].url, base());
    assert_eq!(t.calls()[0].method, "PUT");
    assert_eq!(t.calls()[0].body(), &json!({ "add_labels": "ui,p1" }));
    assert_eq!(
        added,
        json!({ "summary": "Labels on #7 in g/r: bug, p1, ui.", "number": 7, "labels": ["bug", "p1", "ui"] })
    );
    let removed = t
        .ok(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["p1"], "action": "remove" }),
        )
        .await;
    assert_eq!(t.calls()[1].body(), &json!({ "remove_labels": "p1" }));
    assert_eq!(removed["labels"], json!(["bug", "ui"]));
    let replaced = t
        .ok(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["wontfix"], "action": "set" }),
        )
        .await;
    assert_eq!(t.calls()[2].body(), &json!({ "labels": "wontfix" }));
    assert_eq!(replaced["labels"], json!(["wontfix"]));
}

#[tokio::test]
async fn issue_label_removes_every_label_with_an_empty_set_and_nothing_else_may_be_empty() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, issue(json!({ "labels": [] })))]);
    let payload = t
        .ok(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": [], "action": "set" }),
        )
        .await;
    assert_eq!(t.calls()[0].body(), &json!({ "labels": "" }));
    assert_eq!(payload["summary"], "Labels on #7 in g/r: none.");
    assert_match(
        &t.refused(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": [] }),
        )
        .await,
        r"at least one label\. Only action set may be empty",
    );
    assert_match(
        &t.refused(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": [], "action": "remove" }),
        )
        .await,
        "at least one label",
    );
}

#[tokio::test]
async fn issue_label_says_which_labels_are_not_as_asked() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "labels": ["Bug"] }))),
        reply(200, issue(json!({ "labels": ["bug", "p1"] }))),
    ]);
    let added = t
        .ok(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["bug", "p2"] }),
        )
        .await;
    assert_eq!(added["not_applied"], json!(["p2"]));
    assert_match(
        added["summary"].as_str().unwrap(),
        r"GitLab did not apply: p2\.$",
    );
    let removed = t
        .ok(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["BUG"], "action": "remove" }),
        )
        .await;
    assert_eq!(removed["not_applied"], json!(["BUG"]));
    assert_match(removed["summary"].as_str().unwrap(), r"GitLab kept: BUG\.$");
}

#[tokio::test]
async fn issue_label_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, issue(json!({})))]);
    assert_match(
        &t.refused("issue_label", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "`labels` is required",
    );
    assert_match(
        &t.refused(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["a,b"] }),
        )
        .await,
        "`labels` may not contain a comma",
    );
    assert_match(
        &t.refused(
            "issue_label",
            json!({ "repo": "g/r", "number": 7, "labels": ["a"], "action": "x" }),
        )
        .await,
        "`action` must be one of add, remove, set",
    );
    assert_match(
        &t.refused("issue_label", json!({ "repo": "g/r", "labels": ["a"] }))
            .await,
        "`number` is required",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ issue_lock */

#[tokio::test]
async fn issue_lock_locks_by_default_and_unlocks_on_request() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, issue(json!({ "discussion_locked": true }))),
        reply(200, issue(json!({ "discussion_locked": false }))),
    ]);
    let locked = t
        .ok("issue_lock", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert_eq!(t.calls()[0].url, base());
    assert_eq!(t.calls()[0].method, "PUT");
    assert_eq!(t.calls()[0].body(), &json!({ "discussion_locked": true }));
    assert_eq!(
        locked,
        json!({ "summary": "Locked #7 in g/r.", "number": 7, "locked": true })
    );
    let unlocked = t
        .ok(
            "issue_lock",
            json!({ "repo": "g/r", "number": 7, "locked": false }),
        )
        .await;
    assert_eq!(t.calls()[1].body(), &json!({ "discussion_locked": false }));
    assert_eq!(
        unlocked,
        json!({ "summary": "Unlocked #7 in g/r.", "number": 7, "locked": false })
    );
}

#[tokio::test]
async fn issue_lock_reports_the_state_gitlab_answered_with_and_refuses_bad_arguments() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        issue(json!({ "discussion_locked": null })),
    )]);
    assert_match(
        &t.refused(
            "issue_lock",
            json!({ "repo": "g/r", "number": 7, "locked": "yes" }),
        )
        .await,
        "`locked` must be true or false",
    );
    assert_match(
        &t.refused("issue_lock", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_eq!(t.call_count(), 0);
    let payload = t
        .ok("issue_lock", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert_eq!(
        payload["summary"],
        "GitLab answered, but #7 in g/r is still unlocked."
    );
    assert_eq!(payload["locked"], false);
}
