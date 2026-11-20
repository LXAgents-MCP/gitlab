//! commit_read, commit_list and commit_compare, and the diff helpers they share.
use lxagents_gitlab::config::config::{MAX_COMMITS_SHOWN, MAX_DIFF_CHARS, MAX_FILES_SHOWN};
use lxagents_gitlab::utils::diffs::{count_lines, file_status, summarize_file, unified_diff};
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const PARENT: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// A commit as GitLab returns it.
fn commit(overrides: Value) -> Value {
    with(
        json!({
            "id": SHA, "short_id": &SHA[..8], "title": "Add login", "message": "Add login\n\nWith a form.\n",
            "author_name": "Ada", "author_email": "ada@example.com",
            "authored_date": "2026-10-01T12:00:00.000+00:00", "committed_date": "2026-10-01T12:00:00.000+00:00",
            "created_at": "2026-10-01T12:00:00.000+00:00", "parent_ids": [PARENT],
            "web_url": format!("https://gitlab.com/g/r/-/commit/{SHA}"),
            "stats": { "additions": 4, "deletions": 1, "total": 5 },
        }),
        overrides,
    )
}

/// Drop keys, as `undefined` in an override did.
fn without(mut value: Value, keys: &[&str]) -> Value {
    for key in keys {
        value.as_object_mut().unwrap().remove(*key);
    }
    value
}

/// A changed file as GitLab returns it in a diff.
fn changed(overrides: Value) -> Value {
    with(
        json!({
            "old_path": "src/a.js", "new_path": "src/a.js", "new_file": false, "renamed_file": false,
            "deleted_file": false, "diff": "@@ -1,2 +1,3 @@\n keep\n-old\n+new\n+more\n",
        }),
        overrides,
    )
}

/* ------------------------------------------------------------------ diffs */

#[test]
fn diffs_a_files_status_its_line_counts_and_the_unified_text_with_the_headers_gitlab_leaves_out() {
    assert_eq!(file_status(&json!({ "new_file": true })), "added");
    assert_eq!(file_status(&json!({ "deleted_file": true })), "removed");
    assert_eq!(file_status(&json!({ "renamed_file": true })), "renamed");
    assert_eq!(file_status(&json!({})), "modified");

    assert_eq!(
        count_lines(&json!(
            "@@ -1 +1 @@\n-a\n-b\n+c\n \\ No newline at end of file\n"
        )),
        (1, 2)
    );
    assert_eq!(
        count_lines(&json!("@@ -1 +1 @@\n--- a\n+++ b\n")),
        (1, 1),
        "a line that begins with dashes is still a line"
    );
    assert_eq!(count_lines(&Value::Null), (0, 0));

    assert_eq!(
        summarize_file(&changed(json!({}))),
        json!({ "filename": "src/a.js", "status": "modified", "additions": 2, "deletions": 1 })
    );

    assert_eq!(
        unified_diff(&[changed(
            json!({ "old_path": "a", "new_path": "b", "renamed_file": true, "diff": "@@ -1 +1 @@\n-x\n+y\n" })
        )]),
        "diff --git a/a b/b\n--- a/a\n+++ b/b\n@@ -1 +1 @@\n-x\n+y\n"
    );
    assert_eq!(
        unified_diff(&[changed(
            json!({ "old_path": "n", "new_path": "n", "new_file": true, "diff": "@@ -0,0 +1 @@\n+z\n" })
        )]),
        "diff --git a/n b/n\n--- /dev/null\n+++ b/n\n@@ -0,0 +1 @@\n+z\n"
    );
    assert_match(
        &unified_diff(&[changed(
            json!({ "deleted_file": true, "diff": "", "too_large": true }),
        )]),
        r"\+\+\+ /dev/null\n\(GitLab left this file's diff out because it is too large\)\n$",
    );
}

/* ------------------------------------------------------------------ commit_read */

#[tokio::test]
async fn commit_read_reads_a_commit_and_the_files_it_changed_from_two_requests() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, commit(json!({}))),
        reply_with(
            200,
            json!([changed(json!({})), changed(json!({ "old_path": "n", "new_path": "n", "new_file": true, "diff": "@@ -0,0 +1 @@\n+z\n" }))]),
            &[("x-next-page", "")],
        ),
    ]);
    let payload = t
        .ok("commit_read", json!({ "repo": "g/r", "ref": "feat/x" }))
        .await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].url,
        format!("{API}/projects/g%2Fr/repository/commits/feat%2Fx")
    );
    assert_eq!(
        calls[1].path(),
        "/api/v4/projects/g%2Fr/repository/commits/feat%2Fx/diff"
    );
    assert_eq!(json!(calls[1].query()), json!({ "per_page": "100" }));
    assert_eq!(
        payload,
        json!({
            "summary": "Commit 0123456 by Ada: Add login",
            "sha": SHA, "message": "Add login\n\nWith a form.\n", "author": "Ada",
            "date": "2026-10-01T12:00:00.000+00:00", "parents": [PARENT], "additions": 4, "deletions": 1,
            "file_count": 2,
            "files": [
                { "filename": "src/a.js", "status": "modified", "additions": 2, "deletions": 1 },
                { "filename": "n", "status": "added", "additions": 1, "deletions": 0 },
            ],
            "truncated": false, "html_url": format!("https://gitlab.com/g/r/-/commit/{SHA}"),
        })
    );
}

#[tokio::test]
async fn commit_read_defaults_to_head_and_adds_the_diff_when_asked_cut_at_the_limit() {
    let t = Harness::with_token();
    let big = changed(json!({ "diff": format!("@@ -1 +1 @@\n{}", "+x\n".repeat(MAX_DIFF_CHARS)) }));
    t.mock(vec![
        reply(200, commit(json!({}))),
        reply(200, json!([changed(json!({}))])),
        reply(200, commit(json!({}))),
        reply(200, json!([big])),
    ]);

    let small = t
        .ok("commit_read", json!({ "repo": "g/r", "diff": true }))
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/commits/HEAD")
    );
    assert_eq!(
        small["diff"],
        "diff --git a/src/a.js b/src/a.js\n--- a/src/a.js\n+++ b/src/a.js\n@@ -1,2 +1,3 @@\n keep\n-old\n+new\n+more\n"
    );
    assert_eq!(small["truncated"], false);

    let cut = t
        .ok("commit_read", json!({ "repo": "g/r", "diff": true }))
        .await;
    assert_eq!(cut["truncated"], true);
    assert_match(cut["diff"].as_str().unwrap(), r"\.\.\. \(truncated\)$");
}

#[tokio::test]
async fn commit_read_says_when_there_are_more_files_than_it_lists_and_totals_from_the_files_without_stats()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, without(commit(json!({})), &["stats"])),
        reply_with(200, json!([changed(json!({}))]), &[("x-next-page", "2")]),
    ]);
    let payload = t.ok("commit_read", json!({ "repo": "g/r" })).await;
    assert_eq!(payload["truncated"], true);
    assert_eq!(
        (payload["additions"].clone(), payload["deletions"].clone()),
        (json!(2), json!(1))
    );
    assert_eq!(MAX_FILES_SHOWN, 100);
}

#[tokio::test]
async fn commit_read_refuses_a_missing_commit_a_body_that_is_not_one_and_bad_arguments_before_any_request()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 Commit Not Found" })),
        reply(200, json!({ "nope": true })),
    ]);
    assert_match(
        &t.refused("commit_read", json!({ "repo": "g/r", "ref": "a b" }))
            .await,
        "`ref` 'a b' is not a valid",
    );
    assert_match(
        &t.refused("commit_read", json!({ "repo": "g/r", "diff": "yes" }))
            .await,
        "`diff` must be true or false",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("commit_read", json!({ "repo": "g/r", "ref": "nope" }))
            .await,
        r"could not find what it needs to read the commit 'nope' of g/r \(404\): 404 Commit Not Found",
    );
    assert_match(
        &t.refused("commit_read", json!({ "repo": "g/r" })).await,
        "not a commit this server recognizes",
    );
}

/* ------------------------------------------------------------------ commit_list */

#[tokio::test]
async fn commit_list_sends_the_filters_and_the_paging_and_lists_each_commit() {
    let t = Harness::with_token();
    t.mock(vec![reply_with(
        200,
        json!([
            commit(json!({})),
            without(
                commit(json!({ "id": PARENT, "title": "Start" })),
                &["author_name", "authored_date"]
            )
        ]),
        &[("x-next-page", "3")],
    )]);
    let payload = t
        .ok(
            "commit_list",
            json!({
                "repo": "g/r", "ref": "release/1.0", "path": "/src/", "author": " Ada ", "since": "2026-10-01",
                "until": "2026-10-09T10:00:00Z", "page": 2, "per_page": 2,
            }),
        )
        .await;
    let call = &t.calls()[0];
    assert_eq!(call.path(), "/api/v4/projects/g%2Fr/repository/commits");
    assert_eq!(
        json!(call.query()),
        json!({ "ref_name": "release/1.0", "path": "src", "author": "Ada", "since": "2026-10-01", "until": "2026-10-09T10:00:00Z", "page": "2", "per_page": "2" })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "2 commits on release/1.0 of g/r on page 2.",
            "items": [
                { "sha": SHA, "message": "Add login", "author": "Ada", "date": "2026-10-01T12:00:00.000+00:00" },
                { "sha": PARENT, "message": "Start" },
            ],
            "page": 2, "per_page": 2, "has_more": true,
        })
    );
}

#[tokio::test]
async fn commit_list_sends_no_filters_by_default_reads_the_link_header_and_reports_an_empty_page() {
    let t = Harness::with_token();
    t.mock(vec![
        reply_with(
            200,
            json!([commit(json!({}))]),
            &[("link", r#"<https://gitlab.com/x?page=2>; rel="next""#)],
        ),
        reply(200, json!([])),
    ]);
    let some = t.ok("commit_list", json!({ "repo": "g/r" })).await;
    assert_eq!(
        json!(t.calls()[0].query()),
        json!({ "page": "1", "per_page": "30" })
    );
    assert_eq!(some["summary"], "1 commit of g/r on page 1.");
    assert_eq!(some["has_more"], true);
    let none = t
        .ok(
            "commit_list",
            json!({ "repo": "g/r", "ref": "dev", "page": 9 }),
        )
        .await;
    assert_eq!(none["summary"], "No commits on dev of g/r match on page 9.");
}

#[tokio::test]
async fn commit_list_refuses_bad_arguments_before_any_request() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!([]))]);
    assert_match(
        &t.refused(
            "commit_list",
            json!({ "repo": "g/r", "since": "yesterday" }),
        )
        .await,
        "`since` must be an ISO 8601 date",
    );
    assert_match(
        &t.refused(
            "commit_list",
            json!({ "repo": "g/r", "until": "2026-13-45" }),
        )
        .await,
        "`until` must be an ISO 8601 date",
    );
    assert_match(
        &t.refused("commit_list", json!({ "repo": "g/r", "path": "../x" }))
            .await,
        "not a path inside the repository",
    );
    assert_match(
        &t.refused("commit_list", json!({ "repo": "g/r", "ref": "a..b" }))
            .await,
        r"`ref` 'a\.\.b' is not a valid",
    );
    assert_match(
        &t.refused("commit_list", json!({ "repo": "g/r", "author": 4 }))
            .await,
        "`author` must be a string",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ commit_compare */

#[tokio::test]
async fn commit_compare_asks_both_ways_to_say_how_far_ahead_and_behind_and_lists_commits_and_files()
{
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            json!({
                "commit": commit(json!({})),
                "commits": [commit(json!({ "id": SHA, "created_at": "2026-10-02T00:00:00Z" })), commit(json!({ "id": PARENT, "title": "Start", "created_at": "2026-10-01T00:00:00Z" }))],
                "diffs": [changed(json!({}))], "compare_timeout": false, "compare_same_ref": false,
                "web_url": "https://gitlab.com/g/r/-/compare/main...feat",
            }),
        ),
        reply(200, json!({ "commit": null, "commits": [{ "id": "c" }], "diffs": [] })),
    ]);
    let payload = t
        .ok(
            "commit_compare",
            json!({ "repo": "g/r", "base": "main", "head": "feat/x" }),
        )
        .await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        json!(calls[0].query()),
        json!({ "from": "main", "to": "feat/x" })
    );
    assert_eq!(
        json!(calls[1].query()),
        json!({ "from": "feat/x", "to": "main" })
    );
    assert_eq!(
        payload,
        json!({
            "summary": "feat/x has diverged from main: 2 ahead, 1 behind.",
            "status": "diverged", "ahead_by": 2, "behind_by": 1, "total_commits": 2,
            "commits": [{ "sha": PARENT, "message": "Start", "author": "Ada" }, { "sha": SHA, "message": "Add login", "author": "Ada" }],
            "file_count": 1,
            "files": [{ "filename": "src/a.js", "status": "modified", "additions": 2, "deletions": 1 }],
            "truncated": false, "html_url": "https://gitlab.com/g/r/-/compare/main...feat",
        })
    );
}

#[tokio::test]
async fn commit_compare_says_identical_ahead_and_behind_in_a_sentence() {
    let t = Harness::with_token();
    let none = json!({ "commits": [], "diffs": [] });
    let one = json!({ "commits": [commit(json!({}))], "diffs": [] });
    t.mock(vec![
        reply(200, none.clone()),
        reply(200, none.clone()),
        reply(200, one.clone()),
        reply(200, none.clone()),
        reply(200, none),
        reply(200, one),
    ]);
    let args = json!({ "repo": "g/r", "base": "a", "head": "b" });
    assert_eq!(
        t.ok("commit_compare", args.clone()).await["summary"],
        "b and a are identical."
    );
    assert_eq!(
        t.ok("commit_compare", args.clone()).await["summary"],
        "b is 1 commit ahead of a."
    );
    assert_eq!(
        t.ok("commit_compare", args).await["summary"],
        "b is 1 commit behind a."
    );
}

#[tokio::test]
async fn commit_compare_keeps_the_newest_commits_and_the_first_files_and_says_it_did() {
    let t = Harness::with_token();
    let mut many: Vec<Value> = (0..MAX_COMMITS_SHOWN + 5)
        .map(|i| {
            commit(json!({ "id": format!("c{i}"), "title": format!("commit {i}"), "created_at": format!("2026-10-01T{:02}:{:02}:00.000Z", i / 60, i % 60) }))
        })
        .collect();
    many.reverse();
    let files: Vec<Value> = (0..MAX_FILES_SHOWN + 3)
        .map(|i| changed(json!({ "old_path": format!("f{i}"), "new_path": format!("f{i}") })))
        .collect();
    t.mock(vec![
        reply(
            200,
            json!({ "commits": many, "diffs": files, "compare_timeout": false }),
        ),
        reply(200, json!({ "commits": [], "diffs": [] })),
    ]);
    let payload = t
        .ok(
            "commit_compare",
            json!({ "repo": "g/r", "base": "a", "head": "b" }),
        )
        .await;
    let commits = payload["commits"].as_array().unwrap();
    assert_eq!(payload["total_commits"], json!(MAX_COMMITS_SHOWN + 5));
    assert_eq!(commits.len(), MAX_COMMITS_SHOWN);
    assert_eq!(
        commits.last().unwrap()["message"],
        json!(format!("commit {}", MAX_COMMITS_SHOWN + 4))
    );
    assert_eq!(commits[0]["message"], "commit 5");
    assert_eq!(payload["file_count"], json!(MAX_FILES_SHOWN + 3));
    assert_eq!(payload["files"].as_array().unwrap().len(), MAX_FILES_SHOWN);
    assert_eq!(payload["truncated"], true);
}

#[tokio::test]
async fn commit_compare_marks_a_comparison_gitlab_timed_out_on_as_incomplete() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            json!({ "commits": [commit(json!({}))], "diffs": [], "compare_timeout": true }),
        ),
        reply(200, json!({ "commits": [], "diffs": [] })),
    ]);
    assert_eq!(
        t.ok(
            "commit_compare",
            json!({ "repo": "g/r", "base": "a", "head": "b" })
        )
        .await["truncated"],
        true
    );
}

#[tokio::test]
async fn commit_compare_explains_a_missing_ref_refuses_a_body_that_is_not_a_comparison_and_bad_arguments()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(404, json!({ "message": "404 invalid revision Not Found" })),
        reply(200, json!({ "nope": true })),
    ]);
    assert_match(
        &t.refused("commit_compare", json!({ "repo": "g/r", "head": "b" }))
            .await,
        "`base` is required",
    );
    assert_match(
        &t.refused("commit_compare", json!({ "repo": "g/r", "base": "a" }))
            .await,
        "`head` is required",
    );
    assert_match(
        &t.refused(
            "commit_compare",
            json!({ "repo": "g/r", "base": "a b", "head": "b" }),
        )
        .await,
        "`base` 'a b' is not a valid",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "commit_compare",
            json!({ "repo": "g/r", "base": "a", "head": "ghost" }),
        )
        .await,
        r"could not find 'a' or 'ghost' in g/r \(404\).*Check both names",
    );
    assert_match(
        &t.refused(
            "commit_compare",
            json!({ "repo": "g/r", "base": "a", "head": "b" }),
        )
        .await,
        "not a comparison this server recognizes",
    );
}
