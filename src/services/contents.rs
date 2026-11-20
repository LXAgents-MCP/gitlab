//! The repository files API, which the file tools share.
//!
//! GitLab changes a file by `POST`, `PUT` or `DELETE` against its path, one commit on a branch that must
//! be named, so a tool that is not handed a branch has to find the default one first. That lookup, the
//! path, and the encoding of what is written live here once.
use std::sync::LazyLock;

use base64::Engine;
use regex::Regex;

use crate::config::config::MAX_WRITE_BYTES;
use crate::models::address::{FilePath, Repo};
use crate::utils::errors::Refused;

/// The API path of a file: its path written as one encoded segment.
pub fn files_path(repo: &Repo, file: &FilePath) -> String {
    format!("{}/repository/files/{}", repo.path, file.encoded)
}

/// Content as the API wants it written, refusing what is too large to be a source file. It is sent as
/// base64, so that the text arrives exactly as it was given, line endings included.
pub fn encode_content(content: &str) -> Result<String, Refused> {
    let bytes = content.as_bytes();
    if bytes.len() > MAX_WRITE_BYTES {
        return Err(Refused(format!(
            "`content` is {} bytes, over the {MAX_WRITE_BYTES}-byte limit for one write. This tool writes \
             source files; commit large files with git.",
            bytes.len()
        )));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

static EXISTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)already exists").expect("pattern"));
static MISSING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)doesn't exist|does not exist|not found").expect("pattern"));
static CHANGED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)changed since").expect("pattern"));

/// The wording of the 400 GitLab answers a file commit with, which names the cause only in its message.
/// `detail` is GitLab's own message, as a suffix; `None` leaves it to the general wording.
pub fn explain_commit(detail: &str, repo: &Repo, file: &FilePath, branch: &str) -> Option<String> {
    let (path, full) = (&file.path, &repo.full);
    if EXISTS.is_match(detail) {
        return Some(format!(
            "A file '{path}' already exists on {branch} in {full}{detail}. Use file_update to change it."
        ));
    }
    if MISSING.is_match(detail) {
        return Some(format!(
            "There is no file '{path}' on {branch} in {full}{detail}. Use file_create to make it, or directory_list to find it."
        ));
    }
    if CHANGED.is_match(detail) {
        return Some(format!(
            "'{path}' on {branch} in {full} changed since it was read{detail}. Read it again with file_read and use its new last_commit_id."
        ));
    }
    None
}
