//! tag_check, tag_get, tag_get_all, tag_delete, release_create, release_read, release_list and
//! release_delete.
use lxagents_gitlab::controllers::get;
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const TAG_OBJECT: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// A lightweight tag as GitLab returns it.
fn light(overrides: Value) -> Value {
    with(
        json!({
            "name": "v1.0.0", "message": null, "target": COMMIT,
            "commit": { "id": COMMIT, "short_id": &COMMIT[..8], "title": "Release" },
            "release": null, "protected": false, "created_at": null,
        }),
        overrides,
    )
}

/// An annotated tag: its target is the tag object, not the commit.
fn annotated(overrides: Value) -> Value {
    with(
        light(
            json!({ "message": "Version 1.0.0", "target": TAG_OBJECT, "created_at": "2026-10-01T10:00:00.000+00:00" }),
        ),
        overrides,
    )
}

/// A release as GitLab returns it.
fn release(overrides: Value) -> Value {
    with(
        json!({
            "name": "Version 1.0.0", "tag_name": "v1.0.0", "description": "Notes\n\n- one",
            "created_at": "2026-10-01T10:00:00.000Z", "released_at": "2026-10-01T10:00:00.000Z", "upcoming_release": false,
            "assets": {
                "count": 3,
                "sources": [{ "format": "zip", "url": "https://gitlab.com/g/r/-/archive/v1.0.0/r-v1.0.0.zip" }],
                "links": [{ "id": 1, "name": "binary", "url": "https://example.com/b", "direct_asset_url": "https://gitlab.com/g/r/-/releases/v1.0.0/downloads/binary", "link_type": "other" }],
            },
            "_links": { "self": "https://gitlab.com/g/r/-/releases/v1.0.0" },
        }),
        overrides,
    )
}

/// The facts of `release()`, after `summary`.
fn facts(summary: &str) -> Value {
    json!({
        "summary": summary, "tag": "v1.0.0", "name": "Version 1.0.0", "upcoming": false,
        "html_url": "https://gitlab.com/g/r/-/releases/v1.0.0", "released_at": "2026-10-01T10:00:00.000Z", "body": "Notes\n\n- one",
        "assets": [
            { "name": "binary", "url": "https://gitlab.com/g/r/-/releases/v1.0.0/downloads/binary" },
            { "name": "source.zip", "url": "https://gitlab.com/g/r/-/archive/v1.0.0/r-v1.0.0.zip" },
        ],
    })
}

fn query(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/* ------------------------------------------------------------------ tag_check */

#[tokio::test]
async fn tag_check_says_a_tag_exists_and_whether_it_is_annotated() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, light(json!({}))),
        reply(200, annotated(json!({ "name": "rel/2" }))),
    ]);
    let lightweight = t
        .ok("tag_check", json!({ "repo": "g/r", "tag": "v1.0.0" }))
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/tags/v1.0.0")
    );
    assert_eq!(
        lightweight,
        json!({ "summary": "Tag v1.0.0 exists in g/r (lightweight) and points at 0123456.", "exists": true, "tag": "v1.0.0", "sha": COMMIT, "annotated": false })
    );
    let tagged = t
        .ok("tag_check", json!({ "repo": "g/r", "tag": "rel/2" }))
        .await;
    assert_eq!(
        t.calls()[1].url,
        format!("{API}/projects/g%2Fr/repository/tags/rel%2F2")
    );
    assert_eq!(tagged["annotated"], json!(true));
    assert_eq!(
        tagged["sha"],
        json!(COMMIT),
        "the commit, not the tag object"
    );
}

#[tokio::test]
async fn tag_check_treats_a_missing_tag_as_an_answer_but_a_missing_project_as_a_refusal() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Tag Not Found" })),
        reply(404, json!({ "message": "404 Project Not Found" })),
    ]);
    let outcome = t
        .outcome("tag_check", json!({ "repo": "g/r", "tag": "v9" }))
        .await;
    assert!(outcome.ok);
    assert_eq!(
        payload(&outcome),
        json!({ "summary": "Tag v9 does not exist in g/r.", "exists": false, "tag": "v9" })
    );
    assert_match(
        &t.refused("tag_check", json!({ "repo": "g/gone", "tag": "v9" }))
            .await,
        r"could not find the project g/gone \(404\): 404 Project Not Found\. Check the spelling",
    );
}

#[tokio::test]
async fn tag_check_refuses_a_body_that_is_not_a_tag_and_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({ "nope": true }))]);
    assert_match(
        &t.refused("tag_check", json!({ "repo": "g/r" })).await,
        "`tag` is required",
    );
    assert_match(
        &t.refused("tag_check", json!({ "repo": "g/r", "tag": "a b" }))
            .await,
        "`tag` 'a b' is not a valid",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("tag_check", json!({ "repo": "g/r", "tag": "v1" }))
            .await,
        "not a tag this server recognizes",
    );
}

/* ------------------------------------------------------------------ tag_get */

#[tokio::test]
async fn tag_get_reads_a_lightweight_tag_and_an_annotated_one() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            light(json!({ "release": { "tag_name": "v1.0.0", "description": "x" } })),
        ),
        reply(200, annotated(json!({ "protected": true }))),
    ]);
    let lightweight = t
        .ok("tag_get", json!({ "repo": "g/r", "tag": "v1.0.0" }))
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/tags/v1.0.0")
    );
    assert_eq!(
        lightweight,
        json!({
            "summary": "Tag v1.0.0 is a lightweight tag and points at 0123456.", "tag": "v1.0.0", "sha": COMMIT, "target": COMMIT,
            "annotated": false, "protected": false, "has_release": true,
        })
    );
    let tagged = t
        .ok("tag_get", json!({ "repo": "g/r", "tag": "v1.0.0" }))
        .await;
    assert_eq!(
        tagged["summary"],
        json!("Tag v1.0.0 is annotated and points at 0123456.")
    );
    assert_eq!(
        [
            &tagged["target"],
            &tagged["message"],
            &tagged["date"],
            &tagged["protected"],
            &tagged["has_release"]
        ],
        [
            &json!(TAG_OBJECT),
            &json!("Version 1.0.0"),
            &json!("2026-10-01T10:00:00.000+00:00"),
            &json!(true),
            &json!(false)
        ]
    );
}

#[tokio::test]
async fn tag_get_says_there_is_no_such_tag_and_refuses_a_body_that_is_not_one() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Tag Not Found" })),
        reply(200, json!({})),
    ]);
    assert_match(
        &t.refused("tag_get", json!({ "repo": "g/r", "tag": "v9" }))
            .await,
        r"^There is no tag 'v9' in g/r\. List them with tag_get_all\.$",
    );
    assert_match(
        &t.refused("tag_get", json!({ "repo": "g/r", "tag": "v1" }))
            .await,
        "not a tag this server recognizes",
    );
}

/* ------------------------------------------------------------------ tag_get_all */

#[tokio::test]
async fn tag_get_all_lists_tags_with_the_commit_each_points_at_searches_and_pages() {
    let t = Harness::with_token();
    t.mock(vec![
        reply_with(
            200,
            json!([
                light(json!({ "name": "v2" })),
                light(json!({ "name": "v1", "commit": { "id": TAG_OBJECT } }))
            ]),
            &[("x-next-page", "2")],
        ),
        reply(200, json!([])),
    ]);
    let payload = t
        .ok(
            "tag_get_all",
            json!({ "repo": "g/r", "search": " ^v ", "per_page": 2 }),
        )
        .await;
    assert_eq!(
        t.calls()[0].path(),
        "/api/v4/projects/g%2Fr/repository/tags"
    );
    assert_eq!(
        t.calls()[0].query(),
        query(&[("search", "^v"), ("page", "1"), ("per_page", "2")])
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 tags matching '^v' of g/r on page 1.",
            "items": [{ "name": "v2", "sha": COMMIT }, { "name": "v1", "sha": TAG_OBJECT }],
            "page": 1, "per_page": 2, "has_more": true,
        })
    );
    let none = t
        .ok("tag_get_all", json!({ "repo": "g/r", "page": 4 }))
        .await;
    assert_eq!(none["summary"], json!("g/r has no tags on page 4."));
}

/* ------------------------------------------------------------------ tag_delete */

#[tokio::test]
async fn tag_delete_removes_the_tag_and_says_it_left_releases_alone() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(204, Value::Null),
        reply(403, json!({ "message": "403 Forbidden" })),
    ]);
    let payload = t
        .ok("tag_delete", json!({ "repo": "g/r", "tag": "rel/2" }))
        .await;
    assert_eq!(t.calls()[0].method, "DELETE");
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/tags/rel%2F2")
    );
    assert_eq!(
        payload,
        json!({ "summary": "Deleted tag rel/2 from g/r.", "deleted": true, "tag": "rel/2" })
    );
    assert_match(
        get("tag_delete").unwrap().def.description,
        "does not delete a release",
    );
    assert_match(
        &t.refused("tag_delete", json!({ "repo": "g/r", "tag": "v1" }))
            .await,
        r"^GitLab would not delete the tag 'v1' from g/r \(403\): 403 Forbidden\. It may not exist.*protected",
    );
}

/* ------------------------------------------------------------------ release_create */

#[tokio::test]
async fn release_create_makes_a_release_naming_the_default_branch_for_a_tag_that_might_not_exist() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(201, release(json!({}))),
    ]);
    let payload = t.ok("release_create", json!({ "repo": "g/r", "tag": "v1.0.0", "name": " Version 1.0.0 ", "body": "Notes\n\n- one" })).await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(calls[1].url, format!("{API}/projects/g%2Fr/releases"));
    assert_eq!(
        calls[1].body(),
        &json!({ "tag_name": "v1.0.0", "ref": "main", "name": "Version 1.0.0", "description": "Notes\n\n- one" })
    );
    assert_eq!(
        payload,
        facts("Created release Version 1.0.0 (tag v1.0.0) in g/r.")
    );
}

#[tokio::test]
async fn release_create_takes_a_target_and_a_release_date_and_an_upcoming_release_says_so() {
    let t = Harness::with_token();
    let mut answer =
        release(json!({ "upcoming_release": true, "released_at": "2027-01-01T00:00:00.000Z" }));
    answer.as_object_mut().unwrap().remove("assets");
    t.mock(vec![reply(201, answer)]);
    let payload = t.ok("release_create", json!({ "repo": "g/r", "tag": "v2", "target": "release/2", "released_at": "2027-01-01" })).await;
    assert_eq!(t.call_count(), 1);
    assert_eq!(
        t.calls()[0].body(),
        &json!({ "tag_name": "v2", "ref": "release/2", "released_at": "2027-01-01" })
    );
    assert_eq!(payload["upcoming"], json!(true));
    assert_match(
        payload["summary"].as_str().unwrap(),
        r"^Created upcoming release Version 1\.0\.0",
    );
    assert_eq!(payload["assets"], json!([]));
}

#[tokio::test]
async fn release_create_says_when_the_tag_already_has_a_release_and_refuses_bad_arguments_before_any_request()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(
        409,
        json!({ "message": "Release already exists" }),
    )]);
    assert_match(
        &t.refused("release_create", json!({ "repo": "g/r" })).await,
        "`tag` is required",
    );
    assert_match(
        &t.refused("release_create", json!({ "repo": "g/r", "tag": "a b" }))
            .await,
        "`tag` 'a b' is not a valid",
    );
    assert_match(
        &t.refused(
            "release_create",
            json!({ "repo": "g/r", "tag": "v1", "target": "a..b" }),
        )
        .await,
        "`target` 'a..b' is not a valid",
    );
    assert_match(
        &t.refused(
            "release_create",
            json!({ "repo": "g/r", "tag": "v1", "released_at": "soon" }),
        )
        .await,
        "`released_at` must be an ISO 8601 date",
    );
    assert_match(
        &t.refused(
            "release_create",
            json!({ "repo": "g/r", "tag": "v1", "body": 3 }),
        )
        .await,
        "`body` must be a string",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "release_create",
            json!({ "repo": "g/r", "tag": "v1", "target": "main" }),
        )
        .await,
        r"could not create a release for 'v1' in g/r \(409\): Release already exists\. A release for that tag already exists; read it with release_read",
    );
}

/* ------------------------------------------------------------------ release_read */

#[tokio::test]
async fn release_read_reads_the_release_of_a_tag_and_the_latest_through_its_permanent_address() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, release(json!({}))),
        reply(200, release(json!({ "tag_name": "v1.1.0", "name": null }))),
    ]);
    let by_tag = t
        .ok("release_read", json!({ "repo": "g/r", "tag": "rel/1" }))
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/releases/rel%2F1")
    );
    assert_eq!(by_tag, facts("g/r has release Version 1.0.0 (tag v1.0.0)."));
    let latest = t.ok("release_read", json!({ "repo": "g/r" })).await;
    assert_eq!(
        t.calls()[1].url,
        format!("{API}/projects/g%2Fr/releases/permalink/latest")
    );
    assert_eq!(
        latest["summary"],
        json!("g/r has release v1.1.0 (tag v1.1.0).")
    );
    assert!(latest.get("name").is_none());
}

#[tokio::test]
async fn release_read_says_there_is_no_such_release_or_none_yet() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Not found" })),
        reply(404, json!({ "message": "404 Not found" })),
        reply(200, json!({ "nope": true })),
    ]);
    assert_match(
        &t.refused("release_read", json!({ "repo": "g/r", "tag": "v9" }))
            .await,
        r"^g/r has no release for the tag 'v9'\. List what exists with release_list\.$",
    );
    assert_match(
        &t.refused("release_read", json!({ "repo": "g/r" })).await,
        r"^g/r has no release yet\.",
    );
    assert_match(
        &t.refused("release_read", json!({ "repo": "g/r" })).await,
        "not a release this server recognizes",
    );
}

/* ------------------------------------------------------------------ release_list */

#[tokio::test]
async fn release_list_asks_newest_first_lists_each_release_and_pages() {
    let t = Harness::with_token();
    let mut older = release(json!({ "tag_name": "v0.9.0", "name": null }));
    older.as_object_mut().unwrap().remove("released_at");
    t.mock(vec![
        reply_with(
            200,
            json!([release(json!({ "upcoming_release": true })), older]),
            &[("x-next-page", "2")],
        ),
        reply(200, json!([])),
    ]);
    let payload = t
        .ok("release_list", json!({ "repo": "g/r", "per_page": 2 }))
        .await;
    assert_eq!(t.calls()[0].path(), "/api/v4/projects/g%2Fr/releases");
    assert_eq!(
        t.calls()[0].query(),
        query(&[
            ("order_by", "released_at"),
            ("sort", "desc"),
            ("page", "1"),
            ("per_page", "2")
        ])
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 releases of g/r on page 1.",
            "items": [
                { "tag": "v1.0.0", "name": "Version 1.0.0", "upcoming": true, "released_at": "2026-10-01T10:00:00.000Z" },
                { "tag": "v0.9.0", "upcoming": false },
            ],
            "page": 1, "per_page": 2, "has_more": true,
        })
    );
    let empty = t
        .ok("release_list", json!({ "repo": "g/r", "page": 3 }))
        .await;
    assert_eq!(empty["summary"], json!("g/r has no releases on page 3."));
}

/* ------------------------------------------------------------------ release_delete */

#[tokio::test]
async fn release_delete_removes_the_release_and_keeps_the_tag() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, release(json!({}))),
        reply(404, json!({ "message": "404 Not found" })),
    ]);
    let payload = t
        .ok("release_delete", json!({ "repo": "g/r", "tag": "v1.0.0" }))
        .await;
    assert_eq!(t.calls()[0].method, "DELETE");
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/releases/v1.0.0")
    );
    assert_eq!(
        payload,
        json!({ "summary": "Deleted the release for v1.0.0 of g/r. Its tag was kept.", "deleted": true, "tag": "v1.0.0" })
    );
    assert_match(
        &t.refused("release_delete", json!({ "repo": "g/r", "tag": "v9" }))
            .await,
        r"^g/r has no release for the tag 'v9'\.",
    );
}

#[tokio::test]
async fn release_delete_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused("release_delete", json!({ "repo": "g/r" })).await,
        "`tag` is required",
    );
    assert_match(
        &t.refused("release_delete", json!({ "repo": "g/r", "tag": "a b" }))
            .await,
        "`tag` 'a b' is not a valid",
    );
    assert_eq!(t.call_count(), 0);
}
