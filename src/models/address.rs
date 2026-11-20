//! Reading the arguments that end up in a GitLab URL: a repository, a reference, a path in a repository.
//! Each is validated here and returned with the form it takes in a URL, so `src/services/api.rs` can
//! build a path from nothing that has not passed through this file.
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;

use super::args::{raw_text, required_text};
use crate::utils::errors::Refused;

/// What `encodeURIComponent` leaves alone: letters, digits and `-_.!~*'()`.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// `encodeURIComponent(text)`.
pub fn encode_component(text: &str) -> String {
    utf8_percent_encode(text, COMPONENT).to_string()
}

/// GitLab's own rule for a path segment, of a group or of a project: letters, digits, '_', '-' and '.',
/// not starting with '-' or '.', at most 255 long.
pub fn is_segment(text: &str) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 255
        && (bytes[0].is_ascii_alphanumeric() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// A project or fork path that GitLab would refuse as a project path: one that ends in `.git` or `.atom`.
pub fn has_reserved_suffix(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.ends_with(".git") || lower.ends_with(".atom")
}

// A project sits in a group, which may sit in up to twenty subgroups.
const MAX_GROUP_DEPTH: usize = 20;
const MAX_PATH_DEPTH: usize = MAX_GROUP_DEPTH + 1;

/// Is this the path of a group or subgroup - or of a user's own namespace, which is one segment?
pub fn is_namespace_path(text: &str) -> bool {
    let segments: Vec<&str> = text.split('/').collect();
    segments.len() <= MAX_GROUP_DEPTH && segments.iter().all(|s| is_segment(s))
}

/// A project argument, read: its name as given, and its API prefix, `/projects/<id>`, with a path written
/// as the single URL segment GitLab expects, slashes encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub full: String,
    pub path: String,
}

fn is_numeric_id(text: &str) -> bool {
    let bytes = text.as_bytes();
    (1..=16).contains(&bytes.len()) && bytes[0] != b'0' && bytes.iter().all(u8::is_ascii_digit)
}

/// Read a project argument: its full path, or its numeric id.
pub fn parse_repo(value: &Value) -> Result<Repo, Refused> {
    parse_repo_field(value, "repo")
}

/// Read a project argument under another name.
pub fn parse_repo_field(value: &Value, field: &str) -> Result<Repo, Refused> {
    let text = required_text(value, field)?;
    if is_numeric_id(&text) {
        return Ok(Repo {
            path: format!("/projects/{text}"),
            full: text,
        });
    }
    let parts: Vec<&str> = text.split('/').collect();
    if parts.len() < 2 || parts.len() > MAX_PATH_DEPTH || !parts.iter().all(|p| is_segment(p)) {
        return Err(Refused(format!(
            "`{field}` '{text}' is not a project. Write its full path, such as 'my-group/my-project' or \
             'my-group/sub-group/my-project', or give its numeric id."
        )));
    }
    Ok(Repo {
        path: format!("/projects/{}", encode_component(&text)),
        full: text,
    })
}

/// Characters git forbids in a reference name, plus whitespace and control characters.
fn bad_ref_char(c: char) -> bool {
    (c as u32) <= 0x20 || c == '\u{7f}' || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
}

/// Read a branch, tag or commit argument.
///
/// Strict enough that the value cannot change what path it lands in - no `..`, no empty or dotted
/// segment, nothing git itself refuses - and loose enough to take any name git allows.
pub fn ref_name(value: &Value, field: &str) -> Result<String, Refused> {
    let name = required_text(value, field)?;
    let bad = name.encode_utf16().count() > 255
        || name.chars().any(bad_ref_char)
        || name.contains("..")
        || name.contains("@{")
        || name.starts_with('-')
        || name.ends_with('.')
        || name
            .split('/')
            .any(|s| s.is_empty() || s.starts_with('.') || s.ends_with(".lock"));
    if bad {
        return Err(Refused(format!(
            "`{field}` '{name}' is not a valid branch, tag or commit name."
        )));
    }
    Ok(name)
}

/// An optional reference: `None` when the argument is absent, refused when it is given and not valid.
pub fn optional_ref(value: &Value, field: &str) -> Result<Option<String>, Refused> {
    if value.is_null() {
        Ok(None)
    } else {
        ref_name(value, field).map(Some)
    }
}

/// A reference as it goes into a URL path. GitLab reads `feature/login` as one name, so it is one path
/// segment, the slash encoded, unlike a name in a query string. `name` was accepted by `ref_name`.
pub fn ref_path(name: &str) -> String {
    encode_component(name)
}

/// A path inside a repository: cleaned, and as GitLab's files API wants it in a URL, one segment with the
/// slashes encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePath {
    pub path: String,
    pub encoded: String,
}

/// Read a path inside a repository. `allow_empty` makes the repository root a valid answer.
pub fn file_path(value: &Value, field: &str, allow_empty: bool) -> Result<FilePath, Refused> {
    let raw = raw_text(value, field)?.unwrap_or_default();
    let trimmed = raw
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if trimmed.is_empty() {
        if allow_empty {
            return Ok(FilePath {
                path: String::new(),
                encoded: String::new(),
            });
        }
        return Err(Refused(format!(
            "`{field}` is required: a path inside the repository, such as 'src/index.js'."
        )));
    }
    let control = trimmed.chars().any(|c| (c as u32) <= 0x1f || c == '\u{7f}');
    if control
        || trimmed
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return Err(Refused(format!(
            "`{field}` '{trimmed}' is not a path inside the repository: no empty, '.' or '..' segments."
        )));
    }
    Ok(FilePath {
        encoded: encode_component(&trimmed),
        path: trimmed,
    })
}
