//! The GitLab client, the failure wording, addressing and paging.
use std::collections::BTreeMap;

use futures::FutureExt;
use lxagents_gitlab::config::config::API_TIMEOUT_MS;
use lxagents_gitlab::config::version::VERSION;
use lxagents_gitlab::models::address::{
    file_path, is_segment, parse_repo, parse_repo_field, ref_name, ref_path,
};
use lxagents_gitlab::models::outcome::Fields;
use lxagents_gitlab::models::paging::{PageQuery, page_query, paging};
use lxagents_gitlab::services::api::{Call, gitlab};
use lxagents_gitlab::services::auth::require_token;
use lxagents_gitlab::services::client::{CallOptions, ReqwestBackend, call_api};
use lxagents_gitlab::services::context::TransportError;
use lxagents_gitlab::services::failure::api_detail;
use lxagents_gitlab::utils::redact::redact;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::helpers::*;

/* ------------------------------------------------------------------ the token */

#[test]
fn require_token_returns_the_trimmed_gitlab_key() {
    let t = Harness::new();
    t.set("GITLAB_API_KEY", "  abc123  ");
    assert_eq!(require_token(&t.ctx).unwrap(), "abc123");
}

#[test]
fn require_token_refuses_a_missing_or_blank_token_and_names_the_variable() {
    let t = Harness::new();
    assert_match(
        &require_token(&t.ctx).unwrap_err().0,
        r"^GITLAB_API_KEY is not set\. .*`env` block",
    );
    t.set("GITLAB_API_KEY", "   ");
    assert_match(
        &require_token(&t.ctx).unwrap_err().0,
        "GITLAB_API_KEY is not set",
    );
}

#[test]
fn redact_removes_every_occurrence_of_a_secret_and_tolerates_an_empty_one() {
    assert_eq!(redact("a s3cret b s3cret", "s3cret"), "a *** b ***");
    assert_eq!(redact("nothing here", "s3cret"), "nothing here");
    assert_eq!(redact("kept", ""), "kept");
}

/* ------------------------------------------------------------------ call_api */

fn options(method: &str) -> CallOptions {
    CallOptions {
        method: method.into(),
        ..CallOptions::default()
    }
}

#[tokio::test]
async fn call_api_sends_the_method_headers_and_a_json_body_and_decodes_the_answer() {
    let t = Harness::new();
    t.mock(vec![reply(201, json!({ "ok": true }))]);
    let mut opts = options("POST");
    opts.headers = vec![("Authorization".into(), "Bearer x".into())];
    opts.json = Some(json!({ "a": 1 }));

    let result = call_api(t.api.as_ref(), "https://api.example.com/things", opts)
        .await
        .unwrap();

    assert_eq!(result.status, 201);
    assert_eq!(result.body, json!({ "ok": true }));
    let call = &t.calls()[0];
    assert_eq!(call.url, "https://api.example.com/things");
    assert_eq!(call.method, "POST");
    assert_eq!(
        call.headers,
        vec![
            ("Authorization".to_string(), "Bearer x".to_string()),
            ("Content-Type".into(), "application/json".into())
        ]
    );
    assert_eq!(call.body, Some(json!({ "a": 1 })));
}

#[tokio::test]
async fn call_api_hands_back_the_response_headers_lowercased() {
    let t = Harness::new();
    t.mock(vec![reply_with(200, json!("[]"), &[("X-Next-Page", "2")])]);
    let answer = call_api(
        t.api.as_ref(),
        "https://api.example.com/things",
        options("GET"),
    )
    .await
    .unwrap();
    assert_eq!(
        answer.headers.get("x-next-page").map(String::as_str),
        Some("2")
    );
}

#[tokio::test]
async fn call_api_sends_no_body_and_no_content_type_when_there_is_no_json() {
    let t = Harness::new();
    t.mock(vec![reply(204, Value::Null)]);
    let result = call_api(
        t.api.as_ref(),
        "https://api.example.com/things",
        CallOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.status, 204);
    assert_eq!(result.body, Value::Null);
    let call = &t.calls()[0];
    assert_eq!(call.method, "GET");
    assert!(call.body.is_none());
    assert!(call.headers.is_empty());
}

#[tokio::test]
async fn call_api_hands_back_a_body_that_is_not_json_as_short_text() {
    let t = Harness::new();
    t.mock(vec![reply(502, json!("x".repeat(1000)))]);
    let answer = call_api(
        t.api.as_ref(),
        "https://api.example.com/things",
        options("GET"),
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 502);
    assert_eq!(answer.body, json!("x".repeat(300)));
}

#[tokio::test]
async fn call_api_keeps_the_whole_body_as_text_when_asked_for_it_raw() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!("y".repeat(1000)))]);
    let mut opts = options("GET");
    opts.raw = true;
    let answer = call_api(t.api.as_ref(), "https://api.example.com/log", opts)
        .await
        .unwrap();
    assert_eq!(answer.body, json!("y".repeat(1000)));
}

#[tokio::test]
async fn call_api_still_decodes_an_error_answer_when_the_body_of_a_good_one_is_asked_for_raw() {
    let t = Harness::new();
    t.mock(vec![reply(404, json!({ "message": "404 Not found" }))]);
    let mut opts = options("GET");
    opts.raw = true;
    let answer = call_api(t.api.as_ref(), "https://api.example.com/log", opts)
        .await
        .unwrap();
    assert_eq!(answer.status, 404);
    assert_eq!(answer.body, json!({ "message": "404 Not found" }));
}

/// A one-request-at-a-time HTTP server on loopback that answers each request with `answer(headers)`.
async fn tiny_server(
    answer: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static,
) -> (u16, tokio::sync::mpsc::UnboundedReceiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (seen, received) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0u8; 16_384];
            let mut read = 0;
            while !String::from_utf8_lossy(&buffer[..read]).contains("\r\n\r\n") {
                match socket.read(&mut buffer[read..]).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => read += n,
                }
            }
            let head = String::from_utf8_lossy(&buffer[..read]).to_string();
            let _ = seen.send(head.clone());
            let _ = socket.write_all(&answer(&head)).await;
            let _ = socket.shutdown().await;
        }
    });
    (port, received)
}

#[tokio::test]
async fn call_api_follows_a_redirect_to_another_host_without_sending_the_token_there() {
    // GitLab moves a project by redirecting a read to its new address. The token travels in
    // `Authorization`, which the client drops when the host changes; this holds it to that, over real
    // sockets. Another port of the same address is another origin.
    let (target, mut target_seen) = tiny_server(|_| {
        b"HTTP/1.1 200 OK\r\ncontent-length: 8\r\nconnection: close\r\n\r\nthe body".to_vec()
    })
    .await;
    let (origin, _) = tiny_server(move |_| {
        format!("HTTP/1.1 302 Found\r\nlocation: http://localhost:{target}/elsewhere\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .into_bytes()
    })
    .await;

    let backend = ReqwestBackend::new();
    let mut opts = options("GET");
    opts.raw = true;
    opts.headers = vec![("Authorization".into(), "Bearer secret".into())];
    let answer = call_api(&backend, &format!("http://127.0.0.1:{origin}/start"), opts)
        .await
        .unwrap();

    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, json!("the body"));
    let head = target_seen.recv().await.unwrap().to_lowercase();
    assert!(!head.contains("authorization"), "{head}");
}

#[tokio::test]
async fn call_api_refuses_an_answer_larger_than_the_client_will_read() {
    let (port, _) = tiny_server(|_| {
        let mut answer =
            b"HTTP/1.1 200 OK\r\ncontent-length: 25000000\r\nconnection: close\r\n\r\n".to_vec();
        answer.extend(std::iter::repeat_n(b'a', 25_000_000));
        answer
    })
    .await;
    let mut opts = options("GET");
    opts.raw = true;
    let refused = call_api(
        &ReqwestBackend::new(),
        &format!("http://127.0.0.1:{port}/big"),
        opts,
    )
    .await
    .unwrap_err();
    assert_match(&refused.0, "larger than 20 MB");
}

#[tokio::test]
async fn call_api_turns_a_network_failure_into_a_refusal_with_the_secret_scrubbed() {
    let t = Harness::new();
    t.mock(vec![failure(TransportError::Failed(
        "getaddrinfo ENOTFOUND for s3cret".into(),
    ))]);
    let mut opts = options("GET");
    opts.token = "s3cret".into();
    let refused = call_api(t.api.as_ref(), "https://api.example.com/things", opts)
        .await
        .unwrap_err();
    assert_eq!(
        refused.0,
        "Could not reach api.example.com: getaddrinfo ENOTFOUND for ***. Check the network connection."
    );
}

#[tokio::test]
async fn call_api_turns_a_timeout_into_a_refusal_that_says_how_long_it_waited() {
    let t = Harness::new();
    t.mock(vec![failure(TransportError::Timeout)]);
    let refused = call_api(
        t.api.as_ref(),
        "https://api.example.com/things",
        options("GET"),
    )
    .await
    .unwrap_err();
    assert_match(
        &refused.0,
        &format!(
            r"^api\.example\.com did not answer within {} seconds",
            API_TIMEOUT_MS / 1000
        ),
    );
}

/* ------------------------------------------------------------------ gitlab() */

#[tokio::test]
async fn gitlab_sends_the_token_as_a_bearer_credential_builds_the_query_and_returns_the_answer() {
    let t = Harness::with_token();
    t.mock(vec![reply_with(
        200,
        json!({ "ok": 1 }),
        &[("x-thing", "1")],
    )]);

    let answer = gitlab(
        &t.ctx,
        "GET",
        "/projects/o%2Fr/things",
        Call::new("")
            .query("page", Some(2))
            .query("per_page", Some(30))
            .query("ref_name", Some("main"))
            .query("empty", Some(""))
            .query("missing", None::<String>),
    )
    .await
    .unwrap();

    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, json!({ "ok": 1 }));
    assert_eq!(answer.headers.get("x-thing").map(String::as_str), Some("1"));
    let call = &t.calls()[0];
    assert_eq!(
        call.url,
        "https://gitlab.com/api/v4/projects/o%2Fr/things?page=2&per_page=30&ref_name=main"
    );
    assert_eq!(call.method, "GET");
    assert_eq!(
        call.header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(
        call.header("PRIVATE-TOKEN"),
        None,
        "GitLab's own header is forwarded across hosts"
    );
    assert_eq!(call.header("Accept"), Some("application/json"));
    assert_eq!(
        call.header("User-Agent"),
        Some(format!("lxagents-mcp-gitlab/{VERSION}").as_str())
    );
    assert_eq!(call.header("Content-Type"), None);
}

#[tokio::test]
async fn gitlab_keeps_an_encoded_slash_in_the_path_as_one_segment() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({}))]);
    let path = format!(
        "/projects/{}/repository/branches/{}",
        ref_path("group/sub/project"),
        ref_path("feat/x")
    );
    gitlab(&t.ctx, "GET", &path, Call::new("")).await.unwrap();
    assert_eq!(
        t.calls()[0].url,
        "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproject/repository/branches/feat%2Fx"
    );
}

#[tokio::test]
async fn gitlab_sends_a_json_body_with_its_content_type_and_a_different_accept_when_asked() {
    let t = Harness::with_token();
    t.mock(vec![reply(201, json!({ "id": 1 }))]);
    gitlab(
        &t.ctx,
        "POST",
        "/projects/1/things",
        Call::new("").json(json!({ "a": 1 })).accept("text/plain"),
    )
    .await
    .unwrap();
    let call = &t.calls()[0];
    assert_eq!(call.body, Some(json!({ "a": 1 })));
    assert_eq!(call.header("Content-Type"), Some("application/json"));
    assert_eq!(call.header("Accept"), Some("text/plain"));
}

#[tokio::test]
async fn gitlab_with_no_token_refuses_before_it_sends_anything() {
    let t = Harness::new();
    t.mock(vec![reply(200, json!({}))]);
    let refused = gitlab(&t.ctx, "GET", "/user", Call::new(""))
        .await
        .unwrap_err();
    assert_match(&refused.0, r"^GITLAB_API_KEY is not set\.");
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn gitlab_refuses_a_path_that_could_change_which_endpoint_is_called() {
    let t = Harness::with_token();
    t.mock(vec![reply(200, json!({}))]);
    for path in ["user", "/user?x=1", "/user#frag"] {
        let crashed = std::panic::AssertUnwindSafe(gitlab(&t.ctx, "GET", path, Call::new("")))
            .catch_unwind()
            .await;
        assert!(crashed.is_err(), "{path} was called");
    }
    assert_eq!(t.call_count(), 0);
}

#[tokio::test]
async fn gitlab_returns_a_status_listed_in_allow_instead_of_refusing() {
    let t = Harness::with_token();
    t.mock(vec![reply(404, json!({ "message": "404 Tag Not Found" }))]);
    let answer = gitlab(
        &t.ctx,
        "GET",
        "/projects/1/repository/tags/v9",
        Call::new("").allow(&[404]),
    )
    .await
    .unwrap();
    assert_eq!(answer.status, 404);
}

async fn refusal(t: &Harness, method: &str, path: &str, call: Call<'_>) -> String {
    gitlab(&t.ctx, method, path, call).await.unwrap_err().0
}

#[tokio::test]
async fn status_401_says_the_token_was_rejected() {
    let t = Harness::with_token();
    t.mock(vec![reply(401, json!({ "message": "401 Unauthorized" }))]);
    assert_match(
        &refusal(&t, "GET", "/user", Call::new("read the user")).await,
        r"^GitLab rejected the token \(401\)\. Check that GITLAB_API_KEY holds a valid token",
    );
}

#[tokio::test]
async fn status_403_says_what_permission_is_missing() {
    let t = Harness::with_token();
    t.mock(vec![reply(403, json!({ "message": "403 Forbidden" }))]);
    assert_match(
        &refusal(
            &t,
            "POST",
            "/projects/1/repository/files/a",
            Call::new("write the file 'a' in o/r"),
        )
        .await,
        r"^GitLab refused to write the file 'a' in o/r \(403\): 403 Forbidden\. .*`api` scope.*`read_api`.*role",
    );
}

#[tokio::test]
async fn a_429_with_retry_after_says_how_long_to_wait_and_one_with_only_a_reset_says_when() {
    let t = Harness::with_token();
    let reset = (chrono::Utc::now().timestamp() + 600).to_string();
    t.mock(vec![
        reply_with(
            429,
            json!({ "message": "Retry later" }),
            &[("retry-after", "30")],
        ),
        reply_with(
            429,
            json!({ "message": { "error": "Retry later" } }),
            &[("ratelimit-reset", &reset)],
        ),
        reply(429, json!("Retry later")),
    ]);
    assert_match(
        &refusal(&t, "GET", "/user", Call::new("")).await,
        r"^GitLab is rate limiting this token \(429\): Retry later\. Wait 30 seconds and try again\.$",
    );
    assert_match(
        &refusal(&t, "GET", "/user", Call::new("")).await,
        r"\(429\): error Retry later\. The limit resets at .*in about (9|10|11) minutes\.$",
    );
    assert_match(
        &refusal(&t, "GET", "/user", Call::new("")).await,
        r"\(429\): Retry later\. Wait a minute and try again\.$",
    );
}

#[tokio::test]
async fn status_404_says_to_check_the_spelling_and_that_a_private_project_answers_404() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        404,
        json!({ "message": "404 Project Not Found" }),
    )]);
    assert_match(
        &refusal(
            &t,
            "GET",
            "/projects/o%2Fr",
            Call::new("read the project o/r"),
        )
        .await,
        r"^GitLab could not find what it needs to read the project o/r \(404\): 404 Project Not Found\. .*private project the token cannot see\.$",
    );
}

#[tokio::test]
async fn status_405_points_at_a_project_that_was_renamed_or_moved() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        405,
        json!({ "message": "Non GET methods are not allowed for moved projects" }),
    )]);
    assert_match(
        &refusal(&t, "DELETE", "/projects/o%2Fold", Call::new("delete o/old")).await,
        r"^GitLab refused to delete o/old \(405\): Non GET methods are not allowed for moved projects\. .*current path or its numeric id\.$",
    );
}

#[tokio::test]
async fn statuses_409_400_and_422_carry_gitlabs_reasons_whichever_shape_it_uses() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(409, json!({ "message": "Branch already exists" })),
        reply(400, json!({ "message": { "name": ["has already been taken"], "path": ["has already been taken", "is reserved"] } })),
        reply(422, json!({ "message": ["first", "second"] })),
    ]);
    assert_match(
        &refusal(&t, "POST", "/x", Call::new("create the branch")).await,
        r"could not create the branch \(409\): Branch already exists\. .*read it again",
    );
    assert_match(
        &refusal(&t, "POST", "/x", Call::new("create it")).await,
        r"^GitLab could not create it \(400\): name has already been taken; path has already been taken, is reserved\.$",
    );
    assert_match(
        &refusal(&t, "POST", "/x", Call::new("merge it")).await,
        r"^GitLab could not merge it \(422\): first; second\.$",
    );
}

#[tokio::test]
async fn a_5xx_says_gitlab_failed_and_any_other_status_is_reported_with_its_number() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(503, json!({ "message": "Unavailable" })),
        reply(418, json!({ "message": "Teapot" })),
    ]);
    assert_match(
        &refusal(&t, "GET", "/x", Call::new("read x")).await,
        r"^GitLab failed to read x \(503\): Unavailable\. Try again shortly\.$",
    );
    assert_match(
        &refusal(&t, "GET", "/x", Call::new("read x")).await,
        r"^GitLab answered 418: Teapot when asked to read x\.$",
    );
}

#[tokio::test]
async fn a_tools_own_wording_wins_and_none_falls_through_to_the_general_wording() {
    let t = Harness::with_token();
    t.mock(vec![
        reply(422, json!({ "message": "bad" })),
        reply(404, json!({ "message": "gone" })),
    ]);
    let explain =
        |status: u16, detail: &str| (status == 422).then(|| format!("custom {status}{detail}"));
    assert_match(
        &refusal(&t, "POST", "/x", Call::new("").explain(explain)).await,
        "^custom 422: bad$",
    );
    assert_match(
        &refusal(&t, "GET", "/x", Call::new("find x").explain(explain)).await,
        r"^GitLab could not find what it needs to find x \(404\)",
    );
}

#[tokio::test]
async fn a_failure_never_carries_the_token_even_when_gitlab_echoes_it() {
    let t = Harness::with_token();
    t.mock(vec![reply(
        500,
        json!({ "message": format!("bad token {TOKEN}") }),
    )]);
    let message = refusal(&t, "GET", "/x", Call::new("")).await;
    assert!(
        !message.contains(TOKEN) && message.contains("***"),
        "{message}"
    );
}

#[test]
fn api_detail_reads_each_shape_gitlab_answers_in_and_plain_text() {
    assert_eq!(api_detail(&json!({ "message": "m" })), ": m");
    assert_eq!(api_detail(&json!({ "message": ["a", "b"] })), ": a; b");
    assert_eq!(api_detail(&json!({ "message": [] })), "");
    assert_eq!(
        api_detail(&json!({ "message": { "name": ["taken"], "base": "broken" } })),
        ": name taken; base broken"
    );
    assert_eq!(api_detail(&json!({ "message": {} })), "");
    assert_eq!(
        api_detail(&json!({ "error": "invalid_token" })),
        ": invalid_token"
    );
    assert_eq!(
        api_detail(&json!({ "error": "insufficient_scope", "error_description": "needs api" })),
        ": insufficient_scope (needs api)"
    );
    assert_eq!(api_detail(&json!("plain")), ": plain");
    assert_eq!(
        api_detail(&json!(
            "<!DOCTYPE html><html><title>Just a moment...</title>"
        )),
        ": the answer was a web page and not the API's, such as a security check or an outage"
    );
    assert_eq!(api_detail(&json!("")), "");
    assert_eq!(api_detail(&Value::Null), "");
    assert_eq!(api_detail(&json!({ "other": 1 })), "");
}

/* ------------------------------------------------------------------ addressing */

#[test]
fn parse_repo_reads_a_projects_full_path_and_builds_the_api_prefix_with_the_slashes_encoded() {
    let repo = parse_repo(&json!(" lxagents-mcp/gitlab ")).unwrap();
    assert_eq!(
        (repo.full.as_str(), repo.path.as_str()),
        ("lxagents-mcp/gitlab", "/projects/lxagents-mcp%2Fgitlab")
    );
    let repo = parse_repo(&json!("group/sub.group/sub-2/my_project.v2")).unwrap();
    assert_eq!(
        repo.path,
        "/projects/group%2Fsub.group%2Fsub-2%2Fmy_project.v2"
    );
}

#[test]
fn parse_repo_reads_a_numeric_id_as_it_is() {
    let repo = parse_repo(&json!("278964")).unwrap();
    assert_eq!(
        (repo.full.as_str(), repo.path.as_str()),
        ("278964", "/projects/278964")
    );
    let repo = parse_repo(&json!(" 7 ")).unwrap();
    assert_eq!(
        (repo.full.as_str(), repo.path.as_str()),
        ("7", "/projects/7")
    );
}

#[test]
fn parse_repo_refuses_anything_that_is_not_a_project_path_or_an_id() {
    let bad = [
        "", "  ", "solo", "a/", "/b", "a//b", "a/..", "a/.", "../b", "a/b c", "-a/b", ".a/b",
        "a/b?x=1", "a/b#c", "a b/c", "0", "007", "12abc", "-5", "1.5",
    ];
    for value in bad {
        let refused = parse_repo(&json!(value)).unwrap_err();
        assert!(refused.0.contains("`repo`"), "{value:?}: {}", refused.0);
    }
    assert_match(
        &parse_repo(&Value::Null).unwrap_err().0,
        "`repo` is required",
    );
    assert_match(
        &parse_repo(&json!(5)).unwrap_err().0,
        "`repo` must be a string, got number",
    );
    assert_match(
        &parse_repo_field(&json!("nope"), "target").unwrap_err().0,
        r"`target` 'nope' is not a project\. Write its full path, such as 'my-group/my-project'",
    );
    assert_match(
        &parse_repo(&json!(vec!["g"; 22].join("/"))).unwrap_err().0,
        "is not a project",
    );
    assert_match(
        &parse_repo(&json!(format!("a/{}", "b".repeat(256))))
            .unwrap_err()
            .0,
        "is not a project",
    );
}

#[test]
fn a_segment_is_gitlabs_rule_for_one_part_of_a_path() {
    for value in [
        "a".to_string(),
        "A1".into(),
        "_x".into(),
        "a.b".into(),
        "a-b_c".into(),
        "x".repeat(255),
    ] {
        assert!(is_segment(&value), "{value}");
    }
    for value in [
        "".to_string(),
        "-a".into(),
        ".a".into(),
        "a b".into(),
        "a/b".into(),
        "x".repeat(256),
    ] {
        assert!(!is_segment(&value), "{value}");
    }
}

#[test]
fn ref_name_accepts_what_git_accepts() {
    for value in [
        "main",
        "feature/login",
        "v1.2.3",
        "HEAD",
        "0123456789abcdef0123456789abcdef01234567",
        "release/1.0.0",
        "ünï",
    ] {
        assert_eq!(ref_name(&json!(value), "ref").unwrap(), value);
    }
    assert_eq!(ref_name(&json!("  main "), "ref").unwrap(), "main");
}

#[test]
fn ref_name_refuses_what_could_change_the_path_or_that_git_itself_refuses() {
    let long = "a".repeat(256);
    let bad = [
        "",
        "a..b",
        "a b",
        "-x",
        "a/",
        "/a",
        "a//b",
        ".hidden",
        "a/.hidden",
        "x.lock",
        "a/x.lock",
        "a~1",
        "a^",
        "a:b",
        "a?b",
        "a*b",
        "a[b",
        "a\\b",
        "a@{b",
        "end.",
        "a\nb",
        long.as_str(),
    ];
    for value in bad {
        assert!(ref_name(&json!(value), "ref").is_err(), "{value:?}");
    }
    assert_match(
        &ref_name(&json!("a b"), "branch").unwrap_err().0,
        r"^`branch` 'a b' is not a valid branch, tag or commit name\.$",
    );
}

#[test]
fn ref_path_is_one_path_segment_the_slashes_encoded_as_gitlab_reads_a_branch_name() {
    assert_eq!(ref_path("feature/login"), "feature%2Flogin");
    assert_eq!(ref_path("a#b/c d"), "a%23b%2Fc%20d");
    assert_eq!(ref_path("100%"), "100%25");
    assert_eq!(ref_path("main"), "main");
}

#[test]
fn file_path_cleans_a_path_and_encodes_it_as_the_one_segment_gitlabs_files_api_takes() {
    let cases = [
        ("/src/a b.js/", "src/a b.js", "src%2Fa%20b.js"),
        ("README.md", "README.md", "README.md"),
        ("dir/ünï#.txt", "dir/ünï#.txt", "dir%2F%C3%BCn%C3%AF%23.txt"),
    ];
    for (given, path, encoded) in cases {
        let file = file_path(&json!(given), "path", false).unwrap();
        assert_eq!((file.path.as_str(), file.encoded.as_str()), (path, encoded));
    }
}

#[test]
fn file_path_treats_the_root_as_valid_only_when_allowed_and_refuses_traversal() {
    for given in [Value::Null, json!("/")] {
        let file = file_path(&given, "path", true).unwrap();
        assert_eq!((file.path.as_str(), file.encoded.as_str()), ("", ""));
    }
    assert_match(
        &file_path(&json!(""), "path", false).unwrap_err().0,
        "`path` is required",
    );
    for value in ["../x", "a/../b", "a/./b", "a//b", "..", "a\u{0000}b"] {
        assert_match(
            &file_path(&json!(value), "path", false).unwrap_err().0,
            "is not a path inside the repository",
        );
    }
    assert_match(
        &file_path(&json!(5), "path", false).unwrap_err().0,
        "`path` must be a string",
    );
}

/* ------------------------------------------------------------------ paging */

fn page_of(args: Value) -> Result<(i64, i64), String> {
    page_query(args.as_object().unwrap())
        .map(|q| (q.page, q.per_page))
        .map_err(|r| r.0)
}

#[test]
fn page_query_defaults_to_the_first_page_of_thirty_and_bounds_what_it_is_given() {
    assert_eq!(page_of(json!({})).unwrap(), (1, 30));
    assert_eq!(
        page_of(json!({ "page": 3, "per_page": 100 })).unwrap(),
        (3, 100)
    );
    assert_match(
        &page_of(json!({ "page": 0 })).unwrap_err(),
        "`page` must be between 1 and",
    );
    assert_match(
        &page_of(json!({ "per_page": 101 })).unwrap_err(),
        r"`per_page` must be between 1 and 100, got 101\.",
    );
    assert_match(
        &page_of(json!({ "page": "2" })).unwrap_err(),
        "`page` must be a whole number",
    );
}

fn more(headers: &[(&str, &str)]) -> Value {
    let headers: BTreeMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    paging(
        Fields::new(),
        PageQuery {
            page: 2,
            per_page: 30,
        },
        &headers,
    )
    .into_value()
}

#[test]
fn paging_says_whether_gitlab_has_another_page() {
    assert_eq!(
        more(&[("x-next-page", "3"), ("x-page", "2")]),
        json!({ "page": 2, "per_page": 30, "has_more": true })
    );
    assert_eq!(more(&[("x-next-page", "")])["has_more"], json!(false));
    assert_eq!(
        more(&[(
            "link",
            r#"<https://gitlab.com/x?page=3>; rel="next", <y>; rel="first""#
        )])["has_more"],
        json!(true)
    );
    assert_eq!(
        more(&[("link", r#"<https://gitlab.com/x?page=1>; rel="prev""#)])["has_more"],
        json!(false)
    );
    assert_eq!(more(&[])["has_more"], json!(false));
}
