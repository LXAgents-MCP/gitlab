//! The two secrets and which is for what. MCP_AUTH_TOKEN guards the HTTP door; GITLAB_API_KEY is the key
//! to GitLab. Neither stands in for the other.
use lxagents_gitlab::config::config::{AUTH_ENV, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV};
use lxagents_gitlab::services::auth::*;

use crate::helpers::{Harness, TOKEN, assert_match};

fn access() -> String {
    "0123456789abcdef".repeat(4)
}

#[test]
fn the_variables_are_the_ones_the_owner_named() {
    assert_eq!(TOKEN_ENV, "GITLAB_API_KEY");
    assert_eq!(AUTH_ENV, "MCP_AUTH_TOKEN");
    assert_eq!(MIN_AUTH_TOKEN_LENGTH, 32);
}

#[test]
fn bearer_credential_reads_the_credential_of_a_bearer_header_and_nothing_else() {
    assert_eq!(
        bearer_credential(Some("Bearer abc")).as_deref(),
        Some("abc")
    );
    assert_eq!(
        bearer_credential(Some("bearer    abc")).as_deref(),
        Some("abc")
    );
    assert_eq!(
        bearer_credential(Some("BEARER abc")).as_deref(),
        Some("abc")
    );
    for header in [
        None,
        Some(""),
        Some("Bearer"),
        Some("Bearer "),
        Some("Basic abc"),
        Some("Bearer a b"),
        Some("Token abc"),
        Some("Bearerabc"),
    ] {
        assert_eq!(bearer_credential(header), None, "{header:?}");
    }
}

#[test]
fn the_access_token_is_read_at_call_time_and_trimmed() {
    let t = Harness::new();
    assert_eq!(configured_auth_token(&t.ctx), "");
    t.set(AUTH_ENV, &format!("  {}\n", access()));
    assert_eq!(configured_auth_token(&t.ctx), access());
}

#[test]
fn auth_token_problem_refuses_a_missing_or_short_access_token_and_names_the_variable_never_the_value()
 {
    assert_eq!(
        auth_token_problem("").as_deref(),
        Some("MCP_AUTH_TOKEN is not set")
    );
    assert_eq!(
        auth_token_problem(&"a".repeat(31)).as_deref(),
        Some("MCP_AUTH_TOKEN is 31 characters, and it must be at least 32")
    );
    assert_eq!(auth_token_problem(&"a".repeat(32)), None);
    assert_eq!(auth_token_problem(&access()), None);
    assert!(
        !auth_token_problem("short-secret")
            .unwrap()
            .contains("short-secret")
    );
}

#[test]
fn credential_matches_accepts_the_token_and_nothing_that_merely_resembles_it() {
    let token = access();
    let expected = digest(&token);
    assert!(credential_matches(&token, &expected));
    let wrong = [
        token[..token.len() - 1].to_string(),
        token[1..].to_string(),
        format!("{token}0"),
        token.to_uppercase(),
        String::new(),
        "x".into(),
        "a".repeat(5000),
    ];
    for guess in wrong {
        assert!(!credential_matches(&guess, &expected));
    }
}

#[test]
fn the_gitlab_key_is_the_one_in_gitlab_api_key_read_per_call_and_trimmed() {
    let t = Harness::new();
    t.set(TOKEN_ENV, &format!("  {TOKEN}  "));
    assert_eq!(configured_api_key(&t.ctx), TOKEN);
    assert_eq!(require_token(&t.ctx).unwrap(), TOKEN);
    assert_eq!(token_to_scrub(&t.ctx), TOKEN);

    t.set(TOKEN_ENV, "   ");
    assert_eq!(configured_api_key(&t.ctx), "");
    assert_eq!(token_to_scrub(&t.ctx), "");
    assert_match(
        &require_token(&t.ctx).unwrap_err().0,
        r"^GITLAB_API_KEY is not set\. .*`env` block",
    );
}

#[test]
fn mcp_auth_token_is_not_a_gitlab_key() {
    let t = Harness::new();
    t.set(AUTH_ENV, &access());
    assert_eq!(configured_api_key(&t.ctx), "");
    assert_match(
        &require_token(&t.ctx).unwrap_err().0,
        r"^GITLAB_API_KEY is not set\.",
    );
}

#[test]
fn gitlab_api_key_is_not_an_access_token() {
    let t = Harness::with_token();
    assert_eq!(configured_auth_token(&t.ctx), "");
    assert_eq!(
        auth_token_problem(&configured_auth_token(&t.ctx)).as_deref(),
        Some("MCP_AUTH_TOKEN is not set")
    );
    assert!(!credential_matches(TOKEN, &digest(&access())));
}

#[test]
fn is_loopback_host_knows_this_machine_only_by_its_literal_names() {
    for name in [
        "localhost",
        "LOCALHOST",
        "localhost.",
        " localhost ",
        "127.0.0.1",
        "127.1.2.3",
        "127.255.255.255",
        "::1",
        "[::1]",
    ] {
        assert!(is_loopback_host(name), "{name}");
    }
    for name in [
        "0.0.0.0",
        "::",
        "10.0.0.1",
        "192.168.1.10",
        "128.0.0.1",
        "127.0.0.1.nip.io",
        "localhost.example.com",
        "evil-localhost",
        "127.0.0",
        "127.0.0.256",
        "[::1",
        "",
    ] {
        assert!(!is_loopback_host(name), "{name}");
    }
}
