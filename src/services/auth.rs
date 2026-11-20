//! The two secrets this server deals with, and which one is for what.
//!
//! - **`MCP_AUTH_TOKEN` guards the door.** It is the server's own secret for the HTTP transport: a client
//!   proves it holds it with `Authorization: Bearer <token>`, and a server install - one that listens
//!   where other machines can reach it - refuses to serve HTTP without one. A local install listens on
//!   loopback only and may leave it unset; the `Host` and `Origin` checks in `src/routes/http.rs` then
//!   stand where the token would. stdio is a pipe the client spawns on its own machine, so there is nobody
//!   to authenticate and `src/main.rs` never reads it on that path.
//! - **`GITLAB_API_KEY` is the key to GitLab.** It is the token every GitLab call is made with, on both
//!   transports, read from this process's environment when a tool is called. A client never supplies it
//!   and never sees it, and the bearer a client presents is never sent to GitLab.
use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::context::Ctx;
use crate::config::config::{AUTH_ENV, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV};
use crate::utils::errors::Refused;

/* ------------------------------------------------------------------ the door: MCP_AUTH_TOKEN */

/// The server's access token, or an empty string when there is none.
///
/// Read at call time and never at start, so a value set later is seen. Surrounding whitespace is dropped:
/// a token copied out of a file or an `env_file` often carries a trailing newline, and an HTTP header
/// cannot.
pub fn configured_auth_token(ctx: &Ctx) -> String {
    ctx.var(AUTH_ENV)
}

/// Why `token` cannot guard the HTTP transport, or `None` when it can. The message names the variable and
/// never the value, so it is safe to log.
pub fn auth_token_problem(token: &str) -> Option<String> {
    let length = token.chars().count();
    if token.is_empty() {
        return Some(format!("{AUTH_ENV} is not set"));
    }
    if length < MIN_AUTH_TOKEN_LENGTH {
        return Some(format!(
            "{AUTH_ENV} is {length} characters, and it must be at least {MIN_AUTH_TOKEN_LENGTH}"
        ));
    }
    None
}

/// SHA-256 of a string, so two values of any length compare in constant time.
pub fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^Bearer +(\S+)$").expect("pattern"));

/// The credential in an `Authorization: Bearer <credential>` header, or `None`.
///
/// The scheme is case-insensitive (RFC 7235). Anything else - another scheme, no credential, two of them
/// - is not a bearer credential.
pub fn bearer_credential(header: Option<&str>) -> Option<String> {
    BEARER
        .captures(header.unwrap_or(""))
        .map(|found| found[1].to_string())
}

/// Whether `credential` is the token whose digest is `expected`. The comparison is constant-time over
/// fixed-length digests, so neither the token's length nor how much of it a guess got right shows in how
/// long the answer takes.
pub fn credential_matches(credential: &str, expected: &[u8; 32]) -> bool {
    digest(credential).ct_eq(expected).into()
}

/* ------------------------------------------------------------------ local installs: loopback */

/// Whether `name` is this machine: `localhost`, an address in `127.0.0.0/8`, or `::1`, with or without the
/// brackets a URL puts round an IPv6 address. Used for where the HTTP transport listens, which decides
/// whether it is a local install, and for the host a request says it is addressed to.
///
/// Only these literal forms count. A name that merely resolves to loopback does not, because what it
/// resolves to can change after the check, which is how DNS rebinding works.
pub fn is_loopback_host(name: &str) -> bool {
    let mut host = name.trim().to_lowercase();
    if host.len() >= 2 && host.starts_with('[') && host.ends_with(']') {
        host = host[1..host.len() - 1].to_string();
    }
    if let Some(stripped) = host.strip_suffix('.') {
        host = stripped.to_string();
    }
    if host == "localhost" || host == "::1" {
        return true;
    }
    let Some(rest) = host.strip_prefix("127.") else {
        return false;
    };
    let octets: Vec<&str> = rest.split('.').collect();
    octets.len() == 3
        && octets.iter().all(|octet| {
            (1..=3).contains(&octet.len())
                && octet.bytes().all(|b| b.is_ascii_digit())
                && octet.parse::<u32>().is_ok_and(|n| n <= 255)
        })
}

/* ------------------------------------------------------------------ the key: GITLAB_API_KEY */

/// The GitLab token in `GITLAB_API_KEY`, trimmed, or an empty string.
pub fn configured_api_key(ctx: &Ctx) -> String {
    ctx.var(TOKEN_ENV)
}

/// The text to scrub from anything about to be shown: the GitLab key. Never used to make a call.
pub fn token_to_scrub(ctx: &Ctx) -> String {
    configured_api_key(ctx)
}

/// The GitLab token a call is made with, read when the call is made.
///
/// Read per call and not at startup, so a server without a token still starts and says what is missing
/// when a tool is used. It is never a tool argument, so it never appears in a prompt or a transcript.
pub fn require_token(ctx: &Ctx) -> Result<String, Refused> {
    let token = configured_api_key(ctx);
    if !token.is_empty() {
        return Ok(token);
    }
    Err(Refused(format!(
        "{TOKEN_ENV} is not set. Add it to the `env` block of this server's entry in the client \
         configuration, or to the environment of the HTTP server process, then restart it."
    )))
}
