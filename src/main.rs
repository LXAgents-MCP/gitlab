//! Server entry point.
//!
//! Gives an LLM access to GitLab over MCP, over stdio by default and over HTTP when `MCP_TRANSPORT=http`.
//! Two secrets, neither an argument:
//!
//! - `GITLAB_API_KEY` is the GitLab token, used for every call on both transports.
//! - `MCP_AUTH_TOKEN` is this server's own access token, which an HTTP client must present as
//!   `Authorization: Bearer <token>` (`src/routes/http.rs`). A server install needs it; a local install, on
//!   loopback, does not. stdio never reads it.
//!
//! stdio: the client starts this binary and speaks to it over a pipe; register it with the GitLab token in
//! its `env` block:
//!
//! ```json
//! {"mcpServers": {"lxagents-gitlab": {"command": "lxagents-gitlab", "env": {"GITLAB_API_KEY": "<token>"}}}}
//! ```
//!
//! Nothing here may write to stdout: on stdio, stdout is the JSON-RPC channel.
use lxagents_gitlab::config::version::{SERVER_NAME, VERSION};
use lxagents_gitlab::routes::http::{run_http, shutdown_signal};
use lxagents_gitlab::routes::server::{GitlabServer, install_panic_hook};
use lxagents_gitlab::services::context::Ctx;
use lxagents_gitlab::utils::log::log;
use rmcp::ServiceExt;

const USAGE: &str = "LXAgents GitLab MCP Server

Usage: lxagents-gitlab [options]

Serves GitLab tools over stdio, or over HTTP when MCP_TRANSPORT=http. Neither secret is
an argument:
  GITLAB_API_KEY  the GitLab token; set it in the environment of this process, read when a tool
                  is called. Used on both transports.
  MCP_AUTH_TOKEN  http only: the server's access token, at least 32 characters, which every
                  request must carry as Authorization: Bearer <token>. Required for a server
                  install, where other machines can reach HTTP; optional for a local install,
                  on loopback. stdio does not read it.
  PORT (default 3000) and HOST say where HTTP listens. HOST defaults to 0.0.0.0, every
  interface, when MCP_AUTH_TOKEN is set, and to 127.0.0.1, this machine only, when it is not.

Options:
  -h, --help       Show this help
  -v, --version    Show the version
";

enum Flag {
    Help,
    Version,
    Serve,
}

/// The flags, as Node's `parseArgs` read them in strict mode: no positionals, no unknown option.
fn parse_flags(args: &[String]) -> Result<Flag, String> {
    let (mut help, mut version) = (false, false);
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "-v" | "--version" => version = true,
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("Unknown option '{other}'"));
            }
            other => {
                return Err(format!(
                    "Unexpected argument '{other}'. This command does not take positional arguments"
                ));
            }
        }
    }
    Ok(if help {
        Flag::Help
    } else if version {
        Flag::Version
    } else {
        Flag::Serve
    })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_flags(&args) {
        Err(message) => {
            log(&format!("{message}\n\n{USAGE}"));
            std::process::exit(2);
        }
        Ok(Flag::Help) => {
            log(USAGE);
            std::process::exit(0);
        }
        Ok(Flag::Version) => {
            log(&format!("{SERVER_NAME} {VERSION}"));
            std::process::exit(0);
        }
        Ok(Flag::Serve) => {}
    }

    install_panic_hook();
    let ctx = Ctx::production();

    // Which transport this process speaks, selected as `shared-instruction` selects it. stdio is the default.
    let transport = std::env::var("MCP_TRANSPORT")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let transport = if transport.is_empty() {
        "stdio".to_string()
    } else {
        transport
    };

    let code = match transport.as_str() {
        "http" | "streamable-http" => run_http(ctx).await,
        "stdio" => {
            // Serving starts with the client's `initialize`, so the signal handlers and the log line come
            // first: a client that never speaks, or a signal before it does, still ends the process cleanly.
            let signal = shutdown_signal();
            log(&format!("{SERVER_NAME} {VERSION} serving over stdio"));
            tokio::pin!(signal);
            let service = tokio::select! {
                started = GitlabServer::new(ctx).serve(rmcp::transport::stdio()) => started,
                _ = &mut signal => std::process::exit(0),
            };
            match service {
                Ok(service) => {
                    // The client closing stdin ends the session; so does SIGINT or SIGTERM.
                    tokio::select! {
                        _ = service.waiting() => {}
                        _ = &mut signal => {}
                    }
                    0
                }
                // stdin closed before the client initialized: the client went away, which is an ending.
                Err(rmcp::service::ServerInitializeError::ConnectionClosed(_)) => 0,
                Err(error) => {
                    log(&format!(
                        "{SERVER_NAME} {VERSION} could not start over stdio: {error}"
                    ));
                    1
                }
            }
        }
        other => {
            log(&format!(
                "MCP_TRANSPORT must be stdio or http, got '{other}'."
            ));
            2
        }
    };
    std::process::exit(code);
}
