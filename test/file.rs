//! file_read, directory_list, file_create, file_update and file_delete.
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use lxagents_gitlab::config::config::{MAX_FILE_CHARS, MAX_WRITE_BYTES};
use serde_json::{Value, json};

use crate::branch::with;
use crate::helpers::*;

const BLOB: &str = "f30e840422a0b9b0a0e0cc1a11118c6356801cf5";
const LAST: &str = "3458caded2072cc2a9a037bbb4f3b9af842ff8ea";

fn b64(text: &str) -> String {
    STANDARD.encode(text.as_bytes())
}

/// A file as GitLab returns it.
fn file(text: &str, overrides: Value) -> Value {
    with(
        json!({
            "file_name": "index.js", "file_path": "src/index.js", "size": text.len(), "encoding": "base64",
            "content": b64(text), "ref": "HEAD", "blob_id": BLOB, "commit_id": "aaaa", "last_commit_id": LAST,
        }),
        overrides,
    )
}

/* ------------------------------------------------------------------ file_read */

#[tokio::test]
async fn file_read_reads_a_file_at_head_which_is_the_default_branch_with_its_ids() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, file("console.log(1);\n", json!({})))]);
    let payload = t
        .ok(
            "file_read",
            json!({ "repo": "g/r", "path": "/src/index.js" }),
        )
        .await;
    assert_eq!(t.call_count(), 1);
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/files/src%2Findex.js?ref=HEAD")
    );
    assert_eq!(
        payload,
        json!({
            "summary": "Read src/index.js (16 bytes) from g/r.", "path": "src/index.js", "blob_id": BLOB,
            "last_commit_id": LAST, "size": 16, "text": "console.log(1);\n", "truncated": false,
        })
    );
}

#[tokio::test]
async fn file_read_reads_at_a_branch_a_tag_or_a_sha_and_decodes_utf8() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, file("héllo ✓", json!({})))]);
    let payload = t
        .ok(
            "file_read",
            json!({ "repo": "g/r", "path": "a b/ü.txt", "ref": "release/1.0" }),
        )
        .await;
    assert_eq!(
        t.calls()[0].url,
        format!("{API}/projects/g%2Fr/repository/files/a%20b%2F%C3%BC.txt?ref=release%2F1.0")
    );
    assert_eq!(payload["text"], "héllo ✓");
    assert_match(payload["summary"].as_str().unwrap(), r" at release/1\.0\.$");
}

#[tokio::test]
async fn file_read_cuts_a_long_file_at_the_limit_and_says_so() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        file(&"x".repeat(MAX_FILE_CHARS + 500), json!({})),
    )]);
    let payload = t
        .ok("file_read", json!({ "repo": "g/r", "path": "big.txt" }))
        .await;
    assert_eq!(
        payload["text"].as_str().unwrap().chars().count(),
        MAX_FILE_CHARS
    );
    assert_eq!(payload["truncated"], true);
    assert_match(
        payload["summary"].as_str().unwrap(),
        r", cut at 25000 characters\.$",
    );
}

#[tokio::test]
async fn file_read_refuses_a_binary_file_a_file_over_1_mb_and_a_body_that_is_not_a_file() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            file(
                "",
                json!({ "content": STANDARD.encode([0x50, 0x4b, 0x00, 0x03]), "size": 4 }),
            ),
        ),
        reply(200, file("x", json!({ "size": 2_000_000 }))),
        reply(200, json!({ "message": "hello" })),
    ]);
    assert_match(
        &t.refused("file_read", json!({ "repo": "g/r", "path": "a.zip" }))
            .await,
        r"`path` 'a\.zip' is a binary file \(4 bytes\)",
    );
    assert_match(
        &t.refused("file_read", json!({ "repo": "g/r", "path": "big.bin" }))
            .await,
        r"is 2000000 bytes, over the 1000000 this tool reads\. Fetch it with git",
    );
    assert_match(
        &t.refused("file_read", json!({ "repo": "g/r", "path": "x" }))
            .await,
        "not a file this server recognizes",
    );
}

#[tokio::test]
async fn file_read_says_there_is_no_such_file_and_that_a_directory_is_not_a_file() {
    let t = Harness::with_token();
    t.mock(vec![reply(404, json!({ "message": "404 File Not Found" }))]);
    assert_match(
        &t.refused(
            "file_read",
            json!({ "repo": "g/r", "path": "doc", "ref": "main" }),
        )
        .await,
        r"^There is no file 'doc' in g/r at main \(404\): 404 File Not Found\. Check the path with directory_list; a directory cannot be read as a file\.",
    );
}

#[tokio::test]
async fn file_read_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, file("", json!({})))]);
    assert_match(
        &t.refused("file_read", json!({ "repo": "g/r" })).await,
        "`path` is required",
    );
    assert_match(
        &t.refused("file_read", json!({ "repo": "g/r", "path": "../x" }))
            .await,
        "not a path inside the repository",
    );
    assert_match(
        &t.refused(
            "file_read",
            json!({ "repo": "g/r", "path": "a", "ref": "a b" }),
        )
        .await,
        "`ref` 'a b' is not a valid",
    );
    assert_match(
        &t.refused("file_read", json!({ "repo": "nope", "path": "a" }))
            .await,
        "is not a project",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ directory_list */

#[tokio::test]
async fn directory_list_lists_the_root_maps_gitlabs_kinds_to_dir_file_and_submodule_and_pages() {
    let t = Harness::with_token();
    t.mock(vec![reply_with(
        200,
        json!([
            { "id": "1", "name": "src", "type": "tree", "path": "src", "mode": "040000" },
            { "id": "2", "name": "README.md", "type": "blob", "path": "README.md", "mode": "100644" },
            { "id": "3", "name": "vendor", "type": "commit", "path": "vendor", "mode": "160000" },
        ]),
        &[("x-next-page", "2")],
    )]);
    let payload = t
        .ok("directory_list", json!({ "repo": "g/r", "per_page": 3 }))
        .await;
    let call = &t.calls()[0];
    assert_eq!(call.path(), "/api/v4/projects/g%2Fr/repository/tree");
    assert_eq!(json!(call.query()), json!({ "page": "1", "per_page": "3" }));
    assert_eq!(
        payload,
        json!({
            "summary": "3 entries in the root of g/r, on page 1.", "path": "",
            "items": [
                { "name": "src", "type": "dir", "path": "src" },
                { "name": "README.md", "type": "file", "path": "README.md" },
                { "name": "vendor", "type": "submodule", "path": "vendor" },
            ],
            "page": 1, "per_page": 3, "has_more": true,
        })
    );
}

#[tokio::test]
async fn directory_list_sends_the_path_and_the_ref_and_says_when_a_page_is_empty() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            200,
            json!([{ "name": "a.js", "type": "blob", "path": "src/tools/a.js" }]),
        ),
        reply(200, json!([])),
    ]);
    let one = t
        .ok(
            "directory_list",
            json!({ "repo": "g/r", "path": "/src/tools/", "ref": "dev", "page": 1 }),
        )
        .await;
    assert_eq!(
        json!(t.calls()[0].query()),
        json!({ "path": "src/tools", "ref": "dev", "page": "1", "per_page": "30" })
    );
    assert_eq!(
        one["summary"],
        "1 entry in 'src/tools' of g/r at dev, on page 1."
    );
    let none = t
        .ok(
            "directory_list",
            json!({ "repo": "g/r", "path": "src", "page": 5 }),
        )
        .await;
    assert_eq!(none["summary"], "No entries in 'src' of g/r on page 5.");
    assert_eq!(none["has_more"], false);
}

#[tokio::test]
async fn directory_list_says_there_is_no_such_directory_and_refuses_bad_arguments_before_any_request()
 {
    let t = Harness::with_token();
    t.mock(vec![reply(
        404,
        json!({ "message": "404 invalid revision or path Not Found" }),
    )]);
    assert_match(
        &t.refused("directory_list", json!({ "repo": "g/r", "path": "../x" }))
            .await,
        "not a path inside the repository",
    );
    assert_match(
        &t.refused("directory_list", json!({ "repo": "g/r", "ref": "a b" }))
            .await,
        "`ref` 'a b' is not a valid",
    );
    assert_match(
        &t.refused("directory_list", json!({ "repo": "g/r", "per_page": 101 }))
            .await,
        "`per_page` must be between 1 and 100",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused("directory_list", json!({ "repo": "g/r", "path": "nope" }))
            .await,
        r"^There is no directory 'nope' in g/r \(404\): 404 invalid revision or path Not Found\. Check the path and the ref; a file cannot be listed\.",
    );
}

/* ------------------------------------------------------------------ file_create */

#[tokio::test]
async fn file_create_finds_the_default_branch_then_commits_the_content_as_base64() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(201, json!({ "file_path": "docs/a.md", "branch": "main" })),
    ]);
    let payload = t.ok("file_create", json!({ "repo": "g/r", "path": "docs/a.md", "content": "héllo\r\n", "message": " add a " })).await;
    let calls = t.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].url, format!("{API}/projects/g%2Fr"));
    assert_eq!(calls[1].method, "POST");
    assert_eq!(
        calls[1].url,
        format!("{API}/projects/g%2Fr/repository/files/docs%2Fa.md")
    );
    assert_eq!(
        calls[1].body(),
        &json!({ "branch": "main", "content": b64("héllo\r\n"), "encoding": "base64", "commit_message": "add a" })
    );
    assert_eq!(
        payload,
        json!({ "summary": "Created docs/a.md in g/r on main.", "path": "docs/a.md", "branch": "main" })
    );
}

#[tokio::test]
async fn file_create_commits_to_the_branch_it_is_given_with_no_lookup_and_allows_an_empty_file() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        201,
        json!({ "file_path": ".keep", "branch": "feat/x" }),
    )]);
    let payload = t.ok("file_create", json!({ "repo": "g/r", "path": ".keep", "content": "", "message": "keep", "branch": "feat/x" })).await;
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].body()["branch"], "feat/x");
    assert_eq!(calls[0].body()["content"], "");
    assert_eq!(payload["branch"], "feat/x");
}

#[tokio::test]
async fn file_create_says_when_the_file_exists_and_carries_gitlabs_reason_for_any_other_refusal() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(
            400,
            json!({ "message": "A file with this name already exists" }),
        ),
        reply(
            400,
            json!({ "message": "You are not allowed to push into this branch" }),
        ),
    ]);
    let args =
        json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m", "branch": "main" });
    assert_match(
        &t.refused("file_create", args.clone()).await,
        r"^A file 'a' already exists on main in g/r: A file with this name already exists\. Use file_update to change it\.$",
    );
    assert_match(
        &t.refused("file_create", args).await,
        r"^GitLab could not create 'a' in g/r on main \(400\): You are not allowed to push into this branch\.$",
    );
}

#[tokio::test]
async fn file_create_refuses_bad_arguments_too_large_content_and_a_project_with_no_default_branch()
{
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({ "default_branch": null }))]);
    assert_match(
        &t.refused(
            "file_create",
            json!({ "repo": "g/r", "content": "x", "message": "m" }),
        )
        .await,
        "`path` is required",
    );
    assert_match(
        &t.refused(
            "file_create",
            json!({ "repo": "g/r", "path": "a", "message": "m" }),
        )
        .await,
        "`content` is required",
    );
    assert_match(
        &t.refused(
            "file_create",
            json!({ "repo": "g/r", "path": "a", "content": "x" }),
        )
        .await,
        "`message` is required",
    );
    assert_match(
        &t.refused(
            "file_create",
            json!({ "repo": "g/r", "path": "a", "content": 5, "message": "m" }),
        )
        .await,
        "`content` must be a string",
    );
    assert_match(
        &t.refused("file_create", json!({ "repo": "g/r", "path": "a", "content": "x".repeat(MAX_WRITE_BYTES + 1), "message": "m" })).await,
        "over the 1000000-byte limit for one write",
    );
    assert_eq!(t.call_count(), 0);
    assert_match(
        &t.refused(
            "file_create",
            json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m" }),
        )
        .await,
        "has no default branch yet",
    );
}

/* ------------------------------------------------------------------ file_update */

#[tokio::test]
async fn file_update_commits_the_whole_new_content_and_passes_the_last_commit_it_is_given() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        200,
        json!({ "file_path": "a.txt", "branch": "dev" }),
    )]);
    let payload = t
        .ok("file_update", json!({ "repo": "g/r", "path": "a.txt", "content": "two", "message": "update", "branch": "dev", "last_commit_id": LAST }))
        .await;
    let calls = t.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "PUT");
    assert_eq!(
        calls[0].url,
        format!("{API}/projects/g%2Fr/repository/files/a.txt")
    );
    assert_eq!(
        calls[0].body(),
        &json!({ "branch": "dev", "content": b64("two"), "encoding": "base64", "commit_message": "update", "last_commit_id": LAST })
    );
    assert_eq!(
        payload,
        json!({ "summary": "Updated a.txt in g/r on dev.", "path": "a.txt", "branch": "dev" })
    );
}

#[tokio::test]
async fn file_update_sends_no_last_commit_id_by_default_finds_the_default_branch_and_explains_a_missing_or_stale_file()
 {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(200, json!({ "file_path": "a", "branch": "main" })),
        reply(400, json!({ "message": "A file with this name doesn't exist" })),
        reply(400, json!({ "message": "You are attempting to update or delete a file that has changed since you started editing it" })),
    ]);
    t.ok(
        "file_update",
        json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m" }),
    )
    .await;
    assert!(t.calls()[1].body().get("last_commit_id").is_none());
    assert_match(
        &t.refused(
            "file_update",
            json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m", "branch": "main" }),
        )
        .await,
        r"^There is no file 'a' on main in g/r: A file with this name doesn't exist\. Use file_create to make it",
    );
    assert_match(
        &t.refused("file_update", json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m", "branch": "main", "last_commit_id": LAST })).await,
        r"changed since it was read.*Read it again with file_read and use its new last_commit_id\.$",
    );
}

#[tokio::test]
async fn file_update_refuses_bad_arguments_before_any_request() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused(
            "file_update",
            json!({ "repo": "g/r", "path": "a", "content": "x" }),
        )
        .await,
        "`message` is required",
    );
    assert_match(
        &t.refused("file_update", json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m", "last_commit_id": 5 })).await,
        "`last_commit_id` must be a string",
    );
    assert_match(
        &t.refused(
            "file_update",
            json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m", "branch": "a b" }),
        )
        .await,
        "`branch` 'a b' is not a valid",
    );
    assert_eq!(t.call_count(), 0);
}

/* ------------------------------------------------------------------ file_delete */

#[tokio::test]
async fn file_delete_commits_the_removal_with_a_json_body_and_says_what_it_removed() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, Value::Null)]);
    let payload =
        t.ok("file_delete", json!({ "repo": "g/r", "path": "old.txt", "message": "drop it", "branch": "main", "last_commit_id": LAST })).await;
    let call = &t.calls()[0];
    assert_eq!(call.method, "DELETE");
    assert_eq!(
        call.url,
        format!("{API}/projects/g%2Fr/repository/files/old.txt")
    );
    assert_eq!(
        call.body(),
        &json!({ "branch": "main", "commit_message": "drop it", "last_commit_id": LAST })
    );
    assert_eq!(
        payload,
        json!({ "summary": "Deleted old.txt from g/r on main.", "deleted": true, "path": "old.txt", "branch": "main" })
    );
}

#[tokio::test]
async fn file_delete_finds_the_default_branch_explains_a_missing_file_and_refuses_bad_arguments() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(200, json!({ "default_branch": "main" })),
        reply(204, Value::Null),
        reply(
            400,
            json!({ "message": "A file with this name doesn't exist" }),
        ),
    ]);
    t.ok(
        "file_delete",
        json!({ "repo": "g/r", "path": "a", "message": "m" }),
    )
    .await;
    assert_eq!(t.calls()[1].body()["branch"], "main");
    assert!(t.calls()[1].body().get("last_commit_id").is_none());
    assert_match(
        &t.refused(
            "file_delete",
            json!({ "repo": "g/r", "path": "a", "message": "m", "branch": "main" }),
        )
        .await,
        "^There is no file 'a' on main in g/r",
    );
    let before = t.call_count();
    assert_match(
        &t.refused("file_delete", json!({ "repo": "g/r", "path": "a" }))
            .await,
        "`message` is required",
    );
    assert_match(
        &t.refused("file_delete", json!({ "repo": "g/r", "message": "m" }))
            .await,
        "`path` is required",
    );
    assert_eq!(t.call_count(), before);
}

#[tokio::test]
async fn the_write_tools_are_held_to_the_no_token_rule_only_after_their_arguments_check_out() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!({}))]);
    assert_match(
        &t.refused(
            "file_create",
            json!({ "repo": "g/r", "path": "a", "content": "x", "message": "m", "branch": "main" }),
        )
        .await,
        r"^GITLAB_API_KEY is not set\.",
    );
    assert_eq!(t.call_count(), 0);
}
