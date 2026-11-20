//! Every knob this server has, in one place.
//!
//! There are no configuration files and no flags beyond `--help` and `--version`. The only input from
//! outside the tool arguments is two secrets and three settings, all variables: `GITLAB_API_KEY`, the
//! token every GitLab call is made with; `MCP_AUTH_TOKEN`, the secret a client must present to reach the
//! HTTP transport when other machines can reach it; and `MCP_TRANSPORT`, `PORT` and `HOST`
//! (`src/routes/http.rs`). A server without the GitLab key still starts and says what is missing when a
//! tool is used.

/// The environment variable that holds the GitLab token, on both transports. Never an argument, so it
/// never appears in a prompt.
pub const TOKEN_ENV: &str = "GITLAB_API_KEY";

/// The environment variable that holds the server's own access token for HTTP, named as
/// `shared-instruction` names its own. A client presents it as `Authorization: Bearer <token>`. It is not
/// a GitLab credential and is never sent to GitLab; stdio never reads it. It is required of a server
/// install, which listens where other machines can reach it, and optional for a local install, which
/// listens on loopback only; when it is set it is required on every request, wherever the server listens.
pub const AUTH_ENV: &str = "MCP_AUTH_TOKEN";

/// Characters. The shortest access token HTTP will start with, as in `shared-instruction`.
pub const MIN_AUTH_TOKEN_LENGTH: usize = 32;

/// The port and interface of the HTTP transport when `PORT` and `HOST` are not set and `MCP_AUTH_TOKEN`
/// is, the ones `shared-instruction` uses: 3000 on every interface, so a container's published port
/// reaches it. The process holds a GitLab key, so what guards it is the access token, and TLS is the
/// operator's. `HOST=127.0.0.1` narrows it to this machine.
pub const DEFAULT_PORT: u16 = 3000;
pub const DEFAULT_HOST: &str = "0.0.0.0";

/// The interface when neither `HOST` nor `MCP_AUTH_TOKEN` is set: this machine only. That is a local
/// install, which needs no access token because nothing beyond this machine can reach it.
pub const LOCAL_HOST: &str = "127.0.0.1";

/// Bytes. The largest request body the HTTP transport reads: a 1 MB file write is about 1.4 MB as base64.
pub const MAX_BODY_BYTES: usize = 4_000_000;

/// The site, for a link to a page the API does not hand back. gitlab.com only; self-managed instances are
/// not supported.
pub const WEB_BASE: &str = "https://gitlab.com";

/// The REST API.
pub const API_BASE: &str = "https://gitlab.com/api/v4";

/// Milliseconds. How long a call to GitLab may take before the tool gives up on it.
pub const API_TIMEOUT_MS: u64 = 30_000;

/// Bytes. The most of one answer the client will read; beyond it the call is refused.
pub const MAX_RESPONSE_BYTES: usize = 20_000_000;

/// Characters. Past this the model reasons about a partial result as though it were complete.
pub const MAX_OUTPUT: usize = 30_000;

/// Results per page when a call does not say, and the most GitLab allows.
pub const DEFAULT_PER_PAGE: i64 = 30;
pub const MAX_PER_PAGE: i64 = 100;

/// How `branch_merge` waits for GitLab to work out whether a new merge request can be merged: the pause
/// between reads, in milliseconds, and the most reads. GitLab does it in the background.
pub const MERGE_POLL_MS: u64 = 1_000;
pub const MERGE_POLL_TRIES: usize = 10;

/// Characters of a file shown to the model. A file is read to be looked at, not ingested, and the limit
/// sits below `MAX_OUTPUT` so the summary and the fields still fit in the same result.
pub const MAX_FILE_CHARS: usize = 25_000;

/// Bytes of a file `file_read` will take, and of content one write may carry. Source files fit; blobs are
/// not what this is for.
pub const MAX_READ_BYTES: i64 = 1_000_000;
pub const MAX_WRITE_BYTES: usize = 1_000_000;

/// Characters of a diff shown to the model, which leaves room in a result for the rest of it.
pub const MAX_DIFF_CHARS: usize = 20_000;

/// Changed files and commits listed in a result. The totals are always reported.
pub const MAX_FILES_SHOWN: usize = 100;
pub const MAX_COMMITS_SHOWN: usize = 50;

/// Comments returned with an issue or a merge request when they are asked for. The count is always
/// reported.
pub const MAX_COMMENTS_SHOWN: usize = 30;

/// Characters of job logs shown by one `workflow_logs` call, shared between the jobs it reads, and the
/// most jobs it reads at once. A failing job's log ends with the reason, so the end is kept.
pub const MAX_LOG_CHARS: usize = 20_000;
pub const MAX_LOG_JOBS: usize = 5;

/// CI/CD variables one `workflow_run` may carry. A bound of this server's; GitLab and the project set
/// their own.
pub const MAX_PIPELINE_VARIABLES: usize = 25;
