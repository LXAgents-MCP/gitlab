//! Where the HTTP transport listens: `PORT` and `HOST`, read when called.
use super::config::{AUTH_ENV, DEFAULT_HOST, DEFAULT_PORT, LOCAL_HOST};

/// Where the HTTP transport listens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSettings {
    pub port: u16,
    pub host: String,
}

/// Read `PORT` and `HOST`: 3000 when `PORT` is unset or blank, as in `shared-instruction`. When `HOST` is
/// unset or blank it follows the access token: `0.0.0.0` when `MCP_AUTH_TOKEN` is set, as in
/// `shared-instruction`, and `127.0.0.1` when it is not, so a local install needs neither. A `PORT` that
/// is set and is not a port is an error and not a silent 3000, and `0` is accepted and asks the system
/// for a free port.
///
/// `env` looks a variable up; the process environment in production, a map in a test.
pub fn http_settings(env: &dyn Fn(&str) -> Option<String>) -> Result<HttpSettings, String> {
    let raw = env("PORT").unwrap_or_default().trim().to_string();
    let mut port = DEFAULT_PORT;
    if !raw.is_empty() {
        port = parse_port(&raw)
            .ok_or_else(|| format!("PORT must be a whole number from 0 to 65535, got '{raw}'."))?;
    }
    let token = env(AUTH_ENV).unwrap_or_default().trim().to_string();
    let host = env("HOST").unwrap_or_default().trim().to_string();
    let host = if host.is_empty() {
        if token.is_empty() {
            LOCAL_HOST
        } else {
            DEFAULT_HOST
        }
        .to_string()
    } else {
        host
    };
    Ok(HttpSettings { port, host })
}

/// A port as `Number()` and `Number.isInteger()` would take it: digits only, at most 65535.
fn parse_port(raw: &str) -> Option<u16> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse::<u32>().ok().and_then(|n| u16::try_from(n).ok())
}
