//! pr_open, pr_read, pr_comment, pr_close, pr_merge, pr_review_request and pr_approve.
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;
use crate::issue::{note, user};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const MERGE_SHA: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn base() -> String {
    format!("{API}/projects/g%2Fr/merge_requests/7")
}

/// A merge request as GitLab returns it.
fn mr(overrides: Value) -> Value {
    with(
        json!({
            "id": 9001, "iid": 7, "project_id": 3, "title": "Add login", "description": "Adds the login page.",
            "state": "opened", "draft": false, "created_at": "2026-10-01T10:00:00.000Z",
            "updated_at": "2026-10-02T10:00:00.000Z", "merged_at": null, "source_branch": "feat/login",
            "target_branch": "main", "sha": SHA, "labels": ["ui"], "assignees": [user(11, "alice")],
            "reviewers": [user(13, "carol")], "author": user(12, "bob"), "user_notes_count": 2,
            "changes_count": "3", "merge_status": "can_be_merged", "detailed_merge_status": "mergeable",
            "web_url": "https://gitlab.com/g/r/-/merge_requests/7",
        }),
        overrides,
    )
}

fn methods(t: &Harness) -> Vec<String> {
    t.calls().into_iter().map(|c| c.method).collect()
}

/* ------------------------------------------------------------------ pr_open */

#[tokio::test]
async fn pr_open_opens_a_merge_request_into_the_default_branch_as_a_draft_when_asked() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(
            201,
            mr(json!({ "title": "Draft: Add login", "draft": true })),
        ),
    ]);
    let outcome = t
        .outcome("pr_open", json!({ "repo": "g/r", "title": " Add login ", "head": "feat/login", "body": "Adds the login page.", "draft": true }))
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(calls[1].url, format!("{API}/projects/g%2Fr/merge_requests"));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(
        calls[1].body(),
        &json!({ "source_branch": "feat/login", "target_branch": "main", "title": "Draft: Add login", "description": "Adds the login page." })
    );
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Opened draft merge request !7 in g/r: feat/login into main.", "number": 7, "title": "Draft: Add login",
            "state": "opened", "draft": true, "head": "feat/login", "base": "main", "html_url": "https://gitlab.com/g/r/-/merge_requests/7",
        })
    );
}

#[tokio::test]
async fn pr_open_names_the_base_it_was_given_looks_nothing_up_and_does_not_mark_a_draft_twice() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, mr(json!({}))),
        reply(
            201,
            mr(json!({ "title": "Draft: Add login", "draft": true })),
        ),
    ]);
    let plain = t
        .ok(
            "pr_open",
            json!({ "repo": "g/r", "title": "Add login", "head": "feat/login", "base": "develop" }),
        )
        .await;
    assert_eq!(t.call_count(), 1);
    assert_eq!(
        t.calls()[0].body(),
        &json!({ "source_branch": "feat/login", "target_branch": "develop", "title": "Add login" })
    );
    assert_eq!(plain["draft"], json!(false));
    assert_eq!(plain["base"], json!("develop"));
    t.ok("pr_open", json!({ "repo": "g/r", "title": "Draft: Add login", "head": "feat/login", "base": "develop", "draft": true })).await;
    assert_eq!(t.calls()[1].body()["title"], json!("Draft: Add login"));
}

#[tokio::test]
async fn pr_open_says_when_one_is_already_open_and_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(409, json!({ "message": ["Another open merge request already exists for this source branch: !5"] }))]);
    assert_match(
        &t.refused("pr_open", json!({ "repo": "g/r", "head": "a" }))
            .await,
        "`title` is required",
    );
    assert_match(
        &t.refused("pr_open", json!({ "repo": "g/r", "title": "x" }))
            .await,
        "`head` is required",
    );
    assert_match(
        &t.refused(
            "pr_open",
            json!({ "repo": "g/r", "title": "x", "head": "a..b" }),
        )
        .await,
        r"`head` 'a\.\.b' is not a valid",
    );
    assert_match(
        &t.refused(
            "pr_open",
            json!({ "repo": "g/r", "title": "x", "head": "a", "base": "b c" }),
        )
        .await,
        "`base` 'b c' is not a valid",
    );
    assert_match(
        &t.refused(
            "pr_open",
            json!({ "repo": "g/r", "title": "x", "head": "a", "draft": "yes" }),
        )
        .await,
        "`draft` must be true or false",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "pr_open",
            json!({ "repo": "g/r", "title": "x", "head": "a", "base": "main" }),
        )
        .await,
        r"would not open a merge request from 'a' into 'main' in g/r \(409\): Another open merge request already exists.*One is already open for that branch; find it with pr_read",
    );
}

/* ------------------------------------------------------------------ pr_read */

#[tokio::test]
async fn pr_read_reports_a_merge_request_that_can_be_merged() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, mr(json!({})))]);
    let outcome = t
        .outcome("pr_read", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert!(outcome.ok);
    assert_eq!(t.call_count(), 1);
    assert_eq!(t.calls()[0].url, base());
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Merge request !7 in g/r is opened: Add login (feat/login into main).", "number": 7, "title": "Add login",
            "state": "opened", "draft": false, "merged": false, "mergeable": true, "merge_status": "mergeable", "author": "bob",
            "head": "feat/login", "base": "main", "head_sha": SHA, "changed_files": "3", "labels": ["ui"], "assignees": ["alice"],
            "reviewers": ["carol"], "comment_count": 2, "html_url": "https://gitlab.com/g/r/-/merge_requests/7",
            "created_at": "2026-10-01T10:00:00.000Z", "updated_at": "2026-10-02T10:00:00.000Z", "body": "Adds the login page.",
        })
    );
}

#[tokio::test]
async fn pr_read_says_why_a_merge_request_cannot_be_merged_yet_and_when_gitlab_has_not_worked_it_out()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "draft": true, "detailed_merge_status": "draft_status", "merge_status": "cannot_be_merged" }))),
        reply(200, mr(json!({ "detailed_merge_status": "checking", "merge_status": "checking", "changes_count": null }))),
    ]);
    let blocked = t.ok("pr_read", json!({ "repo": "g/r", "number": 7 })).await;
    assert_eq!(blocked["mergeable"], json!(false));
    assert_eq!(blocked["merge_status"], json!("draft_status"));
    assert_eq!(blocked["draft"], json!(true));
    assert_match(
        blocked["summary"].as_str().unwrap(),
        r"is a draft, opened: Add login .* It cannot be merged yet: it is a draft\.$",
    );
    let pending = t.ok("pr_read", json!({ "repo": "g/r", "number": 7 })).await;
    assert!(
        pending.get("mergeable").is_none(),
        "not known yet is not a no"
    );
    assert!(pending.get("changed_files").is_none());
    assert_match(
        pending["summary"].as_str().unwrap(),
        r"still being worked out; ask again in a moment\.$",
    );
}

#[tokio::test]
async fn pr_read_reads_a_merged_request_without_a_verdict_and_falls_back_to_the_older_merge_status()
{
    let t = Harness::with_token();
    let mut older = mr(json!({ "merge_status": "can_be_merged" }));
    older
        .as_object_mut()
        .unwrap()
        .remove("detailed_merge_status");
    t.mock(vec![
        reply(200, mr(json!({ "state": "merged", "merged_at": "2026-10-03T10:00:00.000Z", "detailed_merge_status": "not_open" }))),
        reply(200, older),
    ]);
    let merged = t.ok("pr_read", json!({ "repo": "g/r", "number": 7 })).await;
    assert_eq!(merged["merged"], json!(true));
    assert_eq!(merged["merged_at"], json!("2026-10-03T10:00:00.000Z"));
    assert!(merged.get("mergeable").is_none());
    assert_match(merged["summary"].as_str().unwrap(), "is merged: Add login");
    let older = t.ok("pr_read", json!({ "repo": "g/r", "number": 7 })).await;
    assert_eq!(older["mergeable"], json!(true));
    assert_eq!(older["merge_status"], json!("can_be_merged"));
}

#[tokio::test]
async fn pr_read_lists_the_changed_files_with_line_counts_and_the_comments_people_wrote() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "user_notes_count": 3 }))),
        reply(
            200,
            json!([
                { "old_path": "a.js", "new_path": "a.js", "new_file": false, "renamed_file": false, "deleted_file": false, "diff": "@@ -1,2 +1,3 @@\n a\n-b\n+c\n+d\n" },
                { "old_path": "b.js", "new_path": "b.js", "new_file": true, "renamed_file": false, "deleted_file": false, "diff": "@@ -0,0 +1 @@\n+x\n" },
            ]),
        ),
        reply(
            200,
            json!([note(1, "first", json!({})), note(2, "approved this merge request", json!({ "system": true })), note(3, "second", json!({ "author": null }))]),
        ),
    ]);
    let payload = t
        .ok(
            "pr_read",
            json!({ "repo": "g/r", "number": 7, "files": true, "comments": true }),
        )
        .await;
    let calls = t.calls();
    assert_eq!(
        calls[1].path(),
        "/api/v4/projects/g%2Fr/merge_requests/7/diffs"
    );
    assert_eq!(calls[1].param("per_page").as_deref(), Some("100"));
    assert_eq!(
        calls[2].path(),
        "/api/v4/projects/g%2Fr/merge_requests/7/notes"
    );
    assert_eq!(
        calls[2].param("activity_filter").as_deref(),
        Some("only_comments")
    );
    assert_eq!(calls[2].param("sort").as_deref(), Some("asc"));
    assert_eq!(
        payload["files"],
        json!([
            { "filename": "a.js", "status": "modified", "additions": 2, "deletions": 1 },
            { "filename": "b.js", "status": "added", "additions": 1, "deletions": 0 },
        ])
    );
    assert_eq!(
        payload["comments"],
        json!([
            { "id": 1, "author": "bob", "created_at": "2026-10-01T11:00:00.000Z", "body": "first" },
            { "id": 3, "created_at": "2026-10-01T11:00:00.000Z", "body": "second" },
        ])
    );
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"Showing the first 2 comments\.$",
    );
}

#[tokio::test]
async fn pr_read_asks_for_no_comments_a_merge_request_does_not_have_and_refuses_what_is_not_one() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "user_notes_count": 0 }))),
        reply(200, json!("<html>")),
        reply(404, json!({ "message": "404 Not found" })),
    ]);
    assert_match(
        &t.refused("pr_read", json!({ "repo": "g/r", "number": 0 }))
            .await,
        "`number` must be between 1",
    );
    assert_match(
        &t.refused("pr_read", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_match(
        &t.refused("pr_read", json!({ "repo": "g/r", "number": 7, "files": 1 }))
            .await,
        "`files` must be true or false",
    );
    assert_eq!(t.call_count(), 0);
    let payload = t
        .ok(
            "pr_read",
            json!({ "repo": "g/r", "number": 7, "comments": true }),
        )
        .await;
    assert_eq!(t.call_count(), 1);
    assert!(payload.get("comments").is_none());
    assert_match(
        &t.refused("pr_read", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "body was not a merge request this server recognizes",
    );
    assert_match(
        &t.refused("pr_read", json!({ "repo": "g/r", "number": 99 }))
            .await,
        r"could not find what it needs to read merge request !99 of g/r \(404\)",
    );
}

/* ------------------------------------------------------------------ pr_comment */

#[tokio::test]
async fn pr_comment_adds_a_comment_and_links_to_it() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, note(55, "Looks good", json!({}))),
        reply(201, note(56, "x", json!({}))),
        reply(200, json!({ "unexpected": true })),
    ]);
    let outcome = t
        .outcome(
            "pr_comment",
            json!({ "repo": "g/r", "number": 7, "body": " Looks good\n" }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, format!("{}/notes", base()));
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].body(), &json!({ "body": " Looks good\n" }));
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Commented on merge request !7 in g/r.", "id": 55, "html_url": "https://gitlab.com/g/r/-/merge_requests/7#note_55" })
    );
    let by_id = t
        .ok(
            "pr_comment",
            json!({ "repo": "3", "number": 7, "body": "x" }),
        )
        .await;
    assert!(by_id.get("html_url").is_none());
    assert_match(
        &t.refused(
            "pr_comment",
            json!({ "repo": "g/r", "number": 7, "body": "x" }),
        )
        .await,
        "body was not a comment this server recognizes",
    );
}

#[tokio::test]
async fn pr_comment_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, note(1, "x", json!({})))]);
    assert_match(
        &t.refused("pr_comment", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "`body` is required",
    );
    assert_match(
        &t.refused(
            "pr_comment",
            json!({ "repo": "g/r", "number": 7, "body": "  " }),
        )
        .await,
        "`body` is required and must not be blank",
    );
    assert_match(
        &t.refused("pr_comment", json!({ "repo": "g/r", "body": "x" }))
            .await,
        "`number` is required",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ pr_close */

#[tokio::test]
async fn pr_close_comments_first_then_closes() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, note(1, "Superseded", json!({}))),
        reply(200, mr(json!({ "state": "closed" }))),
    ]);
    let outcome = t
        .outcome(
            "pr_close",
            json!({ "repo": "g/r", "number": 7, "comment": "Superseded" }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, format!("{}/notes", base()));
    assert_eq!(calls[0].body(), &json!({ "body": "Superseded" }));
    assert_eq!(calls[1].url, base());
    assert_eq!(calls[1].method, "PUT");
    assert_eq!(calls[1].body(), &json!({ "state_event": "close" }));
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Closed merge request !7 in g/r without merging.", "number": 7, "state": "closed", "merged": false,
            "html_url": "https://gitlab.com/g/r/-/merge_requests/7",
        })
    );
}

#[tokio::test]
async fn pr_close_without_a_comment_makes_one_request_and_a_blank_comment_is_none() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "state": "closed" }))),
        reply(200, mr(json!({ "state": "closed" }))),
    ]);
    t.ok("pr_close", json!({ "repo": "g/r", "number": 7 }))
        .await;
    t.ok(
        "pr_close",
        json!({ "repo": "g/r", "number": 7, "comment": "  " }),
    )
    .await;
    assert_eq!(methods(&t), ["PUT", "PUT"]);
}

#[tokio::test]
async fn pr_close_says_a_merged_request_cannot_be_closed_that_the_comment_was_posted_and_what_gitlab_left_open()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, note(1, "why", json!({}))),
        reply(405, json!({ "message": "405 Method Not Allowed" })),
        reply(200, mr(json!({ "state": "merged" }))),
    ]);
    assert_match(
        &t.refused(
            "pr_close",
            json!({ "repo": "g/r", "number": 7, "comment": "why" }),
        )
        .await,
        r"would not close merge request !7 of g/r \(405\): 405 Method Not Allowed\. A merge request that is already merged cannot be closed.*The comment had already been posted\.$",
    );
    let payload = t
        .ok("pr_close", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert_eq!(
        payload["summary"],
        json!("GitLab answered, but merge request !7 in g/r is merged.")
    );
    assert_eq!(payload["merged"], json!(true));
}

#[tokio::test]
async fn pr_close_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused("pr_close", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_match(
        &t.refused(
            "pr_close",
            json!({ "repo": "g/r", "number": 7, "comment": 4 }),
        )
        .await,
        "`comment` must be a string",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ pr_merge */

#[tokio::test]
async fn pr_merge_merges_and_reports_the_commit_it_made() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        mr(json!({ "state": "merged", "merge_commit_sha": MERGE_SHA, "squash_commit_sha": null })),
    )]);
    let outcome = t
        .outcome(
            "pr_merge",
            json!({ "repo": "g/r", "number": 7, "message": "Merge login", "sha": SHA }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, format!("{}/merge", base()));
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(
        calls[0].body(),
        &json!({ "merge_commit_message": "Merge login", "sha": SHA })
    );
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Merged merge request !7 in g/r, at fedcba9.", "merged": true, "state": "merged", "sha": MERGE_SHA })
    );
}

#[tokio::test]
async fn pr_merge_squashes_on_request_and_sends_the_message_to_the_squash_commit() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "state": "merged", "merge_commit_sha": null, "squash_commit_sha": MERGE_SHA }))),
        reply(200, mr(json!({ "state": "merged" }))),
    ]);
    let squashed = t
        .ok(
            "pr_merge",
            json!({ "repo": "g/r", "number": 7, "squash": true, "message": "Add login" }),
        )
        .await;
    assert_eq!(
        t.calls()[0].body(),
        &json!({ "squash": true, "squash_commit_message": "Add login" })
    );
    assert_eq!(squashed["sha"], json!(MERGE_SHA));
    let plain = t
        .ok(
            "pr_merge",
            json!({ "repo": "g/r", "number": 7, "squash": false }),
        )
        .await;
    assert_eq!(t.calls()[1].body(), &json!({ "squash": false }));
    assert!(
        plain.get("sha").is_none(),
        "a fast-forward makes no commit of its own"
    );
    assert_match(
        plain["summary"].as_str().unwrap(),
        r"^Merged merge request !7 in g/r\.$",
    );
}

#[tokio::test]
async fn pr_merge_says_when_gitlab_is_still_completing_the_merge_and_when_it_answers_without_merging()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "state": "locked" }))),
        reply(200, mr(json!({ "state": "opened" }))),
    ]);
    let locked = t
        .outcome("pr_merge", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert!(locked.ok);
    assert_eq!(payload(&locked)["merged"], json!(false));
    assert_match(&locked.summary, r"is completing it; read it with pr_read\.");
    let open = t
        .run("pr_merge", json!({ "repo": "g/r", "number": 7 }))
        .await
        .unwrap();
    assert!(!open.ok);
    assert_match(&open.summary, r"without saying it merged \(it is opened\)");
}

#[tokio::test]
async fn pr_merge_asks_gitlab_why_when_it_will_not_merge() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(405, json!({ "message": "405 Method Not Allowed" })),
        reply(
            200,
            mr(json!({ "draft": true, "detailed_merge_status": "draft_status" })),
        ),
        reply(422, json!({ "message": "Branch cannot be merged" })),
        reply(200, mr(json!({ "state": "closed" }))),
        reply(406, json!({ "message": "Branch cannot be merged" })),
        reply(200, mr(json!({ "detailed_merge_status": "checking" }))),
        reply(405, json!({ "message": "405 Method Not Allowed" })),
        reply(200, mr(json!({ "detailed_merge_status": "mergeable" }))),
        reply(405, json!({ "message": "405 Method Not Allowed" })),
        reply(500, json!({ "message": "boom" })),
    ]);
    let args = json!({ "repo": "g/r", "number": 7 });
    assert_match(
        &t.refused("pr_merge", args.clone()).await,
        r"would not merge merge request !7 of g/r \(405\): 405 Method Not Allowed\. It cannot be merged yet: it is a draft\.$",
    );
    assert_eq!(t.calls()[1].method, "GET");
    assert_eq!(t.calls()[1].url, base());
    assert_match(
        &t.refused("pr_merge", args.clone()).await,
        r"\(422\): Branch cannot be merged\. It is closed\.$",
    );
    assert_match(
        &t.refused("pr_merge", args.clone()).await,
        r"\(406\).* still working out whether it can be merged; try again in a moment\.$",
    );
    assert_match(
        &t.refused("pr_merge", args.clone()).await,
        r"GitLab reports it as mergeable, so the project's merge settings refused it\.$",
    );
    assert_match(
        &t.refused("pr_merge", args).await,
        r"\(405\): 405 Method Not Allowed\.$",
    );
}

#[tokio::test]
async fn pr_merge_explains_a_missing_permission_and_a_branch_that_moved() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(401, json!({ "message": "401 Unauthorized" })),
        reply(
            409,
            json!({ "message": "SHA does not match HEAD of source branch" }),
        ),
    ]);
    assert_match(
        &t.refused("pr_merge", json!({ "repo": "g/r", "number": 7 }))
            .await,
        r"would not let this token merge !7 in g/r \(401\): 401 Unauthorized\. Either the token is not valid, or its user may not merge.*Maintainer",
    );
    assert_match(
        &t.refused(
            "pr_merge",
            json!({ "repo": "g/r", "number": 7, "sha": SHA }),
        )
        .await,
        r"did not merge !7 in g/r \(409\): SHA does not match HEAD of source branch\. The branch moved",
    );
}

#[tokio::test]
async fn pr_merge_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused("pr_merge", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_match(
        &t.refused(
            "pr_merge",
            json!({ "repo": "g/r", "number": 7, "squash": "yes" }),
        )
        .await,
        "`squash` must be true or false",
    );
    assert_match(
        &t.refused(
            "pr_merge",
            json!({ "repo": "g/r", "number": 7, "message": 3 }),
        )
        .await,
        "`message` must be a string",
    );
    assert_match(
        &t.refused("pr_merge", json!({ "repo": "g/r", "number": 7, "sha": 3 }))
            .await,
        "`sha` must be a string",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ pr_review_request */

#[tokio::test]
async fn pr_review_request_adds_reviewers_to_those_already_asked() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({ "reviewers": [user(13, "carol")] }))),
        reply(200, json!([user(14, "dave")])),
        reply(
            200,
            mr(json!({ "reviewers": [user(13, "carol"), user(14, "dave")] })),
        ),
    ]);
    let outcome = t
        .outcome(
            "pr_review_request",
            json!({ "repo": "g/r", "number": 7, "reviewers": ["@dave"] }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls[0].url, base());
    assert_eq!(calls[1].url, format!("{API}/users?username=dave"));
    assert_eq!(calls[2].method, "PUT");
    assert_eq!(calls[2].body(), &json!({ "reviewer_ids": [13, 14] }));
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Review requested on !7 in g/r from: carol, dave.", "number": 7, "reviewers": ["carol", "dave"] })
    );
}

#[tokio::test]
async fn pr_review_request_does_not_write_when_they_were_all_asked_already_and_says_who_gitlab_did_not_add()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, mr(json!({}))),
        reply(200, json!([user(13, "Carol")])),
        reply(200, mr(json!({ "reviewers": [] }))),
        reply(200, json!([user(14, "dave")])),
        reply(200, mr(json!({ "reviewers": [] }))),
    ]);
    let again = t
        .ok(
            "pr_review_request",
            json!({ "repo": "g/r", "number": 7, "reviewers": ["carol"] }),
        )
        .await;
    assert_eq!(methods(&t), ["GET", "GET"]);
    assert_eq!(again["reviewers"], json!(["carol"]));
    let dropped = t
        .ok(
            "pr_review_request",
            json!({ "repo": "g/r", "number": 7, "reviewers": ["dave"] }),
        )
        .await;
    assert_eq!(dropped["not_applied"], json!(["dave"]));
    assert_match(
        dropped["summary"].as_str().unwrap(),
        r"from: nobody\. GitLab did not add: dave\.",
    );
}

#[tokio::test]
async fn pr_review_request_refuses_an_unknown_user_before_changing_anything_and_bad_arguments_before_any_request()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(200, mr(json!({}))), reply(200, json!([]))]);
    assert_match(
        &t.refused("pr_review_request", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "`reviewers` is required",
    );
    assert_match(
        &t.refused(
            "pr_review_request",
            json!({ "repo": "g/r", "number": 7, "reviewers": [] }),
        )
        .await,
        "`reviewers` is required and must list at least one",
    );
    assert_match(
        &t.refused(
            "pr_review_request",
            json!({ "repo": "g/r", "reviewers": ["a"] }),
        )
        .await,
        "`number` is required",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "pr_review_request",
            json!({ "repo": "g/r", "number": 7, "reviewers": ["ghost"] }),
        )
        .await,
        "GitLab has no user named 'ghost', so nothing was changed",
    );
    assert!(t.calls().iter().all(|c| c.method == "GET"));
}

/* ------------------------------------------------------------------ pr_approve */

#[tokio::test]
async fn pr_approve_records_the_approval_and_says_how_many_are_still_needed() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, json!({ "iid": 7, "approvals_required": 2, "approvals_left": 1, "approved_by": [{ "user": user(11, "alice") }] })),
        reply(201, json!({ "iid": 7, "approvals_left": 0, "approved_by": [{ "user": user(11, "alice") }, { "user": user(13, "carol") }] })),
        reply(201, json!({ "iid": 7, "approved_by": [] })),
    ]);
    let first = t
        .ok(
            "pr_approve",
            json!({ "repo": "g/r", "number": 7, "sha": SHA }),
        )
        .await;
    assert_eq!(t.calls()[0].url, format!("{}/approve", base()));
    assert_eq!(t.calls()[0].method, "POST");
    assert_eq!(t.calls()[0].body(), &json!({ "sha": SHA }));
    assert_eq!(
        first,
        json!({
            "summary": "Approved merge request !7 in g/r; approved by alice, 1 more needed.", "number": 7,
            "approved_by": ["alice"], "approvals_left": 1,
        })
    );
    let second = t
        .ok("pr_approve", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert!(t.calls()[1].body.is_none(), "no sha, no body");
    assert_match(
        second["summary"].as_str().unwrap(),
        r"approved by alice, carol, no approvals left to wait for\.$",
    );
    let bare = t
        .ok("pr_approve", json!({ "repo": "g/r", "number": 7 }))
        .await;
    assert!(bare.get("approvals_left").is_none());
    assert_match(
        bare["summary"].as_str().unwrap(),
        r"approved by nobody listed\.$",
    );
}

#[tokio::test]
async fn pr_approve_explains_a_refusal_and_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(401, json!({ "message": "401 Unauthorized" })),
        reply(
            409,
            json!({ "message": "SHA does not match HEAD of source branch" }),
        ),
        reply(201, json!({ "unexpected": true })),
    ]);
    assert_match(
        &t.refused("pr_approve", json!({ "repo": "g/r" })).await,
        "`number` is required",
    );
    assert_match(
        &t.refused(
            "pr_approve",
            json!({ "repo": "g/r", "number": 7, "sha": 3 }),
        )
        .await,
        "`sha` must be a string",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("pr_approve", json!({ "repo": "g/r", "number": 7 }))
            .await,
        r"would not record the approval on !7 of g/r \(401\).*forbid approving your own",
    );
    assert_match(
        &t.refused(
            "pr_approve",
            json!({ "repo": "g/r", "number": 7, "sha": SHA }),
        )
        .await,
        r"did not record the approval on !7 of g/r \(409\)",
    );
    assert_match(
        &t.refused("pr_approve", json!({ "repo": "g/r", "number": 7 }))
            .await,
        "body was not an approval this server recognizes",
    );
}
