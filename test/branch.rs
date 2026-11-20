//! branch_create, branch_read, branch_list, branch_delete and branch_merge.
use std::time::Duration;

use lxagents_gitlab::services::merge_status::mergeability;
use serde_json::{Value, json};

use crate::helpers::*;

pub const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
pub const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// Merge `overrides` into `base`.
pub fn with(mut base: Value, overrides: Value) -> Value {
    for (k, v) in overrides.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

/// A branch as GitLab returns it.
fn branch(overrides: Value) -> Value {
    with(
        json!({
            "name": "main",
            "commit": {
                "id": SHA, "short_id": &SHA[..8], "title": "Add login",
                "message": "Add login\n\nWith a form.", "committed_date": "2026-10-01T12:00:00.000+00:00",
            },
            "merged": false, "protected": true, "default": true, "developers_can_push": false,
            "web_url": "https://gitlab.com/g/r/-/tree/main",
        }),
        overrides,
    )
}

/* ------------------------------------------------------------------ branch_create */

#[tokio::test]
async fn branch_create_finds_the_default_branch_then_creates_from_it() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(
            201,
            branch(json!({ "name": "feat/x", "protected": false, "default": false })),
        ),
    ]);
    let payload = t
        .ok(
            "branch_create",
            json!({ "repo": "g/r", "branch": "feat/x" }),
        )
        .await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(
        calls[1].path(),
        "/api/v4/projects/g%2Fr/repository/branches"
    );
    assert_eq!(
        json!(calls[1].query()),
        json!({ "branch": "feat/x", "ref": "main" })
    );
    assert_eq!(
        payload,
        json!({ "summary": "Created branch feat/x at 0123456 in g/r, from main.", "branch": "feat/x", "sha": SHA, "from": "main" })
    );
}

#[tokio::test]
async fn branch_create_starts_from_the_branch_tag_or_sha_it_is_given_with_no_lookup() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(201, branch(json!({ "name": "hotfix" }))),
        reply(201, branch(json!({ "name": "hotfix2" }))),
    ]);
    t.ok(
        "branch_create",
        json!({ "repo": "g/r", "branch": "hotfix", "from": "release/1.0" }),
    )
    .await;
    t.ok(
        "branch_create",
        json!({ "repo": "g/r", "branch": "hotfix2", "from": SHA }),
    )
    .await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].param("ref").as_deref(), Some("release/1.0"));
    assert_eq!(calls[1].param("ref").as_deref(), Some(SHA));
}

#[tokio::test]
async fn branch_create_says_when_the_name_is_taken_the_source_is_missing_or_there_is_no_default_branch()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(400, json!({ "message": "Branch already exists" })),
        reply(400, json!({ "message": "Invalid reference name: nope" })),
        reply(200, json!({ "default_branch": null })),
    ]);
    assert_match(
        &t.refused(
            "branch_create",
            json!({ "repo": "g/r", "branch": "feat/x" }),
        )
        .await,
        r"could not create the branch 'feat/x' \(400\): Branch already exists\. A branch with that name already exists in g/r",
    );
    assert_match(
        &t.refused(
            "branch_create",
            json!({ "repo": "g/r", "branch": "feat/x", "from": "nope" }),
        )
        .await,
        r"\(400\): Invalid reference name: nope\. 'nope' is not a branch, tag or commit of g/r\.",
    );
    assert_match(
        &t.refused(
            "branch_create",
            json!({ "repo": "g/r", "branch": "feat/x" }),
        )
        .await,
        "g/r has no default branch.*file_create",
    );
}

#[tokio::test]
async fn branch_create_refuses_a_body_that_is_not_a_branch_and_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, json!({ "nope": true }))]);
    assert_match(
        &t.refused("branch_create", json!({ "repo": "g/r" })).await,
        "`branch` is required",
    );
    assert_match(
        &t.refused("branch_create", json!({ "repo": "g/r", "branch": "a b" }))
            .await,
        "`branch` 'a b' is not a valid",
    );
    assert_match(
        &t.refused(
            "branch_create",
            json!({ "repo": "g/r", "branch": "ok", "from": "a..b" }),
        )
        .await,
        r"`from` 'a\.\.b' is not a valid",
    );
    assert_match(
        &t.refused("branch_create", json!({ "repo": "nope", "branch": "ok" }))
            .await,
        "is not a project",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "branch_create",
            json!({ "repo": "g/r", "branch": "ok", "from": "main" }),
        )
        .await,
        "not a branch this server recognizes",
    );
}

/* ------------------------------------------------------------------ branch_read */

#[tokio::test]
async fn branch_read_reports_the_tip_its_title_and_date_and_whether_the_branch_is_protected_or_the_default()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(200, branch(json!({})))]);
    let payload = t
        .ok("branch_read", json!({ "repo": "g/r", "branch": "main" }))
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/branches/main")
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Branch main of g/r is at 0123456, the default and is protected.",
            "name": "main", "sha": SHA, "protected": true, "default": true,
            "message": "Add login", "date": "2026-10-01T12:00:00.000+00:00",
        })
    );
}

#[tokio::test]
async fn branch_read_writes_a_slash_in_the_name_as_one_encoded_segment() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        branch(json!({ "name": "feat/a#b", "protected": false, "default": false })),
    )]);
    let payload = t
        .ok(
            "branch_read",
            json!({ "repo": "g/r", "branch": "feat/a#b" }),
        )
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/branches/feat%2Fa%23b")
    );
    assert_eq!(payload["summary"], "Branch feat/a#b of g/r is at 0123456.");
}

#[tokio::test]
async fn branch_read_leaves_out_what_gitlab_did_not_give_and_refuses_a_missing_branch_plainly() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "name": "x", "commit": { "id": SHA } })),
        reply(404, json!({ "message": "404 Branch Not Found" })),
    ]);
    let payload = t
        .ok("branch_read", json!({ "repo": "g/r", "branch": "x" }))
        .await;
    assert!(payload.get("message").is_none());
    assert!(payload.get("date").is_none());
    assert_eq!(payload["protected"], false);
    assert_match(
        &t.refused("branch_read", json!({ "repo": "g/r", "branch": "nope" }))
            .await,
        r"^GitLab could not find what it needs to read the branch 'nope' of g/r \(404\): 404 Branch Not Found\.",
    );
    assert_match(
        &t.refused("branch_read", json!({ "repo": "g/r", "branch": "a b" }))
            .await,
        "`branch` 'a b' is not a valid",
    );
}

/* ------------------------------------------------------------------ branch_list */

#[tokio::test]
async fn branch_list_sends_the_search_and_the_paging_and_lists_each_branch() {
    let t = Harness::with_token();
    t.mock(vec![reply_with(
        200,
        json!([branch(json!({})), branch(json!({ "name": "feat/x", "protected": false, "default": false, "merged": true, "commit": { "id": OTHER } }))]),
        &[("x-next-page", "3")],
    )]);
    let payload = t
        .ok(
            "branch_list",
            json!({ "repo": "g/r", "search": " ^feat ", "page": 2, "per_page": 2 }),
        )
        .await;
    let call = &t.calls()[0];
    assert_eq!(call.path(), "/api/v4/projects/g%2Fr/repository/branches");
    assert_eq!(
        json!(call.query()),
        json!({ "search": "^feat", "page": "2", "per_page": "2" })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 branches matching '^feat' of g/r on page 2.",
            "items": [
                { "name": "main", "sha": SHA, "protected": true, "default": true, "merged": false },
                { "name": "feat/x", "sha": OTHER, "protected": false, "default": false, "merged": true },
            ],
            "page": 2, "per_page": 2, "has_more": true,
        })
    );
}

#[tokio::test]
async fn branch_list_sends_no_search_by_default_and_reports_an_empty_page() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!([])),
        reply(200, json!([branch(json!({}))])),
    ]);
    let empty = t
        .ok("branch_list", json!({ "repo": "g/r", "page": 9 }))
        .await;
    assert!(t.calls()[0].param("search").is_none());
    assert_eq!(empty["summary"], "g/r has no branches on page 9.");
    assert_eq!(empty["has_more"], false);
    let one = t.ok("branch_list", json!({ "repo": "g/r" })).await;
    assert_eq!(one["summary"], "1 branch of g/r on page 1.");
}

#[tokio::test]
async fn branch_list_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!([]))]);
    assert_match(
        &t.refused("branch_list", json!({ "repo": "g/r", "search": 5 }))
            .await,
        "`search` must be a string",
    );
    assert_match(
        &t.refused("branch_list", json!({ "repo": "g/r", "per_page": 101 }))
            .await,
        "`per_page` must be between 1 and 100",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ branch_delete */

#[tokio::test]
async fn branch_delete_removes_the_branch_and_says_what_it_removed() {
    let t = Harness::with_token();
    t.mock(vec![reply(204, Value::Null)]);
    let payload = t
        .ok(
            "branch_delete",
            json!({ "repo": "g/r", "branch": "feat/x" }),
        )
        .await;
    assert_eq!(t.calls()[0].method, "DELETE");
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/branches/feat%2Fx")
    );
    assert_eq!(
        payload,
        json!({ "summary": "Deleted branch feat/x from g/r.", "deleted": true, "branch": "feat/x" })
    );
}

#[tokio::test]
async fn branch_delete_explains_a_refusal_as_a_missing_default_or_protected_branch() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Branch Not Found" })),
        reply(403, json!({ "message": "403 Forbidden" })),
    ]);
    assert_match(
        &t.refused("branch_delete", json!({ "repo": "g/r", "branch": "main" }))
            .await,
        r"^GitLab would not delete the branch 'main' from g/r \(404\): 404 Branch Not Found\. The branch may not exist, or it is the default branch or a protected branch",
    );
    assert_match(
        &t.refused("branch_delete", json!({ "repo": "g/r", "branch": "main" }))
            .await,
        r"\(403\): 403 Forbidden\.",
    );
}

#[tokio::test]
async fn branch_delete_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(204, Value::Null)]);
    assert_match(
        &t.refused("branch_delete", json!({ "repo": "g/r" })).await,
        "`branch` is required",
    );
    assert_match(
        &t.refused(
            "branch_delete",
            json!({ "repo": "g/r", "branch": ".hidden" }),
        )
        .await,
        "not a valid branch",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ branch_merge */

/// A merge request as GitLab returns it.
fn request(overrides: Value) -> Value {
    with(
        json!({
            "id": 900, "iid": 12, "title": "Merge feat/x into main", "state": "opened", "sha": SHA,
            "detailed_merge_status": "mergeable", "merge_status": "can_be_merged",
            "web_url": "https://gitlab.com/g/r/-/merge_requests/12",
        }),
        overrides,
    )
}

fn ahead() -> Value {
    json!({ "commits": [{ "id": OTHER }], "diffs": [{}], "compare_same_ref": false })
}

const MERGE: fn() -> Value = || json!({ "repo": "g/r", "base": "main", "head": "feat/x" });

#[tokio::test]
async fn branch_merge_compares_opens_a_merge_request_accepts_it_and_reports_the_merge_commit() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, ahead()),
        reply(201, request(json!({}))),
        reply(
            200,
            request(json!({ "state": "merged", "merge_commit_sha": OTHER })),
        ),
    ]);
    let outcome = t
        .outcome(
            "branch_merge",
            json!({ "repo": "g/r", "base": "main", "head": "feat/x", "message": " Ship it " }),
        )
        .await;
    assert!(outcome.ok);
    let calls = t.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].path(), "/api/v4/projects/g%2Fr/repository/compare");
    assert_eq!(
        json!(calls[0].query()),
        json!({ "from": "main", "to": "feat/x" })
    );
    assert_eq!(calls[1].method, "POST");
    assert_eq!(calls[1].url, format!("{API}/projects/g%2Fr/merge_requests"));
    assert_eq!(
        calls[1].body(),
        &json!({ "source_branch": "feat/x", "target_branch": "main", "title": "Merge feat/x into main", "remove_source_branch": false })
    );
    assert_eq!(calls[2].method, "PUT");
    assert_eq!(
        calls[2].url,
        format!("{API}/projects/g%2Fr/merge_requests/12/merge")
    );
    assert_eq!(
        calls[2].body(),
        &json!({ "sha": SHA, "should_remove_source_branch": false, "merge_commit_message": "Ship it" })
    );
    assert_eq!(
        payload(&outcome),
        json!({
            "summary": "Merged feat/x into main in g/r, at fedcba9, through merge request !12.",
            "merged": true, "up_to_date": false, "sha": OTHER, "merge_request": 12,
            "html_url": "https://gitlab.com/g/r/-/merge_requests/12",
        })
    );
}

#[tokio::test]
async fn branch_merge_opens_no_merge_request_when_the_base_already_has_the_head() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        json!({ "commits": [], "diffs": [], "compare_same_ref": false }),
    )]);
    let outcome = t.outcome("branch_merge", MERGE()).await;
    assert!(outcome.ok);
    assert_eq!(t.call_count(), 1, "only the comparison was sent");
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Nothing to merge: main already contains feat/x.", "merged": false, "up_to_date": true })
    );
}

#[tokio::test]
async fn branch_merge_waits_while_gitlab_works_out_whether_it_can_merge_then_merges() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, ahead()),
        reply(
            201,
            request(json!({ "detailed_merge_status": "checking", "merge_status": "checking" })),
        ),
        reply(200, request(json!({ "detailed_merge_status": "checking" }))),
        reply(200, request(json!({}))),
        reply(
            200,
            request(json!({ "state": "merged", "merge_commit_sha": OTHER })),
        ),
    ]);
    let payload = t.ok("branch_merge", MERGE()).await;
    let seen: Vec<String> = t
        .calls()
        .iter()
        .map(|c| {
            format!(
                "{} {}",
                c.method,
                c.url.replace(API, "").split('?').next().unwrap()
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            "GET /projects/g%2Fr/repository/compare",
            "POST /projects/g%2Fr/merge_requests",
            "GET /projects/g%2Fr/merge_requests/12",
            "GET /projects/g%2Fr/merge_requests/12",
            "PUT /projects/g%2Fr/merge_requests/12/merge",
        ]
    );
    assert_eq!(
        *t.sleeps.0.lock().unwrap(),
        vec![Duration::from_millis(1000); 2]
    );
    assert_eq!(payload["merged"], true);
}

#[tokio::test]
async fn branch_merge_closes_the_request_it_opened_and_says_why_when_the_branches_conflict() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, ahead()),
        reply(
            201,
            request(
                json!({ "detailed_merge_status": "conflict", "merge_status": "cannot_be_merged" }),
            ),
        ),
        reply(200, request(json!({ "state": "closed" }))),
    ]);
    assert_match(
        &t.refused("branch_merge", MERGE()).await,
        r"^GitLab will not merge 'feat/x' into 'main' in g/r: the branches conflict\. The merge request it opened, !12, was closed again\.$",
    );
    let calls = t.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[2].method, "PUT");
    assert_eq!(
        calls[2].url,
        format!("{API}/projects/g%2Fr/merge_requests/12")
    );
    assert_eq!(calls[2].body(), &json!({ "state_event": "close" }));
}

#[tokio::test]
async fn branch_merge_closes_the_request_when_the_merge_itself_is_refused_and_names_the_cause() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, ahead()),
        reply(201, request(json!({}))),
        reply(405, json!({ "message": "405 Method Not Allowed" })),
        reply(200, request(json!({ "state": "closed" }))),
        reply(200, ahead()),
        reply(201, request(json!({}))),
        reply(
            409,
            json!({ "message": "SHA does not match HEAD of source branch" }),
        ),
        reply(200, request(json!({ "state": "closed" }))),
        reply(200, ahead()),
        reply(201, request(json!({}))),
        reply(401, json!({ "message": "401 Unauthorized" })),
        reply(500, json!({ "message": "boom" })),
    ]);
    assert_match(
        &t.refused("branch_merge", MERGE()).await,
        r"could not merge 'feat/x' into 'main' in g/r \(405\): 405 Method Not Allowed\. It cannot be merged as it stands\. The merge request it opened, !12, was closed again\.$",
    );
    assert_match(
        &t.refused("branch_merge", MERGE()).await,
        r"\(409\): SHA does not match HEAD of source branch\. 'feat/x' moved while the merge was being made; try again\. The merge request it opened, !12, was closed again\.$",
    );
    assert_match(
        &t.refused("branch_merge", MERGE()).await,
        r"\(401\).*Maintainer role.* The merge request it opened, !12, is still open; close it with pr_close\.$",
    );
    assert_eq!(
        t.calls()
            .iter()
            .filter(|c| c.method == "PUT" && c.url.ends_with("/merge"))
            .count(),
        3
    );
}

#[tokio::test]
async fn branch_merge_gives_up_when_gitlab_never_finishes_working_it_out_and_closes_the_request() {
    let t = Harness::with_token();
    let checking = request(json!({ "detailed_merge_status": "checking" }));
    let mut answers = vec![reply(200, ahead()), reply(201, checking.clone())];
    answers.extend((0..10).map(|_| reply(200, checking.clone())));
    answers.push(reply(200, request(json!({ "state": "closed" }))));
    t.mock(answers);
    assert_match(
        &t.refused("branch_merge", MERGE()).await,
        r"^GitLab is still working out whether 'feat/x' can be merged into 'main' in g/r\. The merge request it opened, !12, was closed again\. Try again in a moment\.$",
    );
    assert_eq!(t.call_count(), 13);
}

#[tokio::test]
async fn branch_merge_reports_a_merge_gitlab_is_still_completing_and_one_it_did_not_complete() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, ahead()),
        reply(201, request(json!({}))),
        reply(200, request(json!({ "state": "locked" }))),
        reply(200, ahead()),
        reply(201, request(json!({}))),
        reply(200, request(json!({ "state": "opened" }))),
    ]);
    let locked = t.outcome("branch_merge", MERGE()).await;
    assert!(locked.ok);
    assert_eq!(locked.fields.get("merged"), Some(&json!(false)));
    assert_match(
        &locked.summary,
        r"is completing it; read it with pr_read\.$",
    );
    let open = t.run("branch_merge", MERGE()).await.unwrap();
    assert!(!open.ok);
    assert_match(
        &open.summary,
        r"without saying it merged \(merge request !12 is opened\)",
    );
}

#[tokio::test]
async fn branch_merge_explains_a_missing_branch_and_a_merge_request_that_is_already_open() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Compare Not Found" })),
        reply(200, ahead()),
        reply(409, json!({ "message": ["Another open merge request already exists for this source branch: !5"] })),
    ]);
    assert_match(
        &t.refused(
            "branch_merge",
            json!({ "repo": "g/r", "base": "main", "head": "ghost" }),
        )
        .await,
        r"could not find 'main' or 'ghost' in g/r \(404\).*Check both names",
    );
    assert_match(
        &t.refused("branch_merge", MERGE()).await,
        r"could not open a merge request from 'feat/x' into 'main' in g/r \(409\): Another open merge request already exists for this source branch: !5\. One is already open; merge that one with pr_merge\.",
    );
}

#[tokio::test]
async fn branch_merge_refuses_a_body_that_is_not_a_merge_request_and_bad_arguments_before_any_request()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, ahead()),
        reply(201, json!({ "nope": true })),
    ]);
    assert_match(
        &t.refused("branch_merge", json!({ "repo": "g/r", "head": "x" }))
            .await,
        "`base` is required",
    );
    assert_match(
        &t.refused("branch_merge", json!({ "repo": "g/r", "base": "main" }))
            .await,
        "`head` is required",
    );
    assert_match(
        &t.refused(
            "branch_merge",
            json!({ "repo": "g/r", "base": "a b", "head": "x" }),
        )
        .await,
        "`base` 'a b' is not a valid",
    );
    assert_match(
        &t.refused(
            "branch_merge",
            json!({ "repo": "g/r", "base": "main", "head": "x", "message": 4 }),
        )
        .await,
        "`message` must be a string",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "branch_merge",
            json!({ "repo": "g/r", "base": "main", "head": "x" }),
        )
        .await,
        "not a merge request this server recognizes",
    );
}

#[test]
fn mergeability_reads_the_detailed_status_falls_back_to_the_older_one_and_says_why_a_request_is_not_ready()
 {
    let m = mergeability(&json!({ "detailed_merge_status": "mergeable" }));
    assert_eq!(
        (m.status.as_str(), m.ready, m.pending, m.reason.as_str()),
        ("mergeable", true, false, "GitLab reports it as mergeable")
    );
    for waiting in ["checking", "unchecked", "preparing"] {
        assert!(
            mergeability(&json!({ "detailed_merge_status": waiting })).pending,
            "{waiting}"
        );
    }
    assert_eq!(
        mergeability(&json!({ "detailed_merge_status": "ci_must_pass" })).reason,
        "its pipeline has to pass first"
    );
    assert!(!mergeability(&json!({ "detailed_merge_status": "draft_status" })).ready);
    assert_eq!(
        mergeability(&json!({ "detailed_merge_status": "something_new" })).reason,
        "GitLab reports it as something_new"
    );
    assert!(mergeability(&json!({ "merge_status": "can_be_merged" })).ready);
    assert!(!mergeability(&json!({ "merge_status": "cannot_be_merged" })).ready);
    assert!(mergeability(&json!({ "merge_status": "checking" })).pending);
    assert!(mergeability(&json!({})).pending);
}
