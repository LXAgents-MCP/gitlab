//! Read one text file from a repository.
use base64::Engine;

use super::prelude::*;
use crate::services::contents::files_path;
use crate::utils::text::{cut_text, decode_text, looks_binary};

pub fn def() -> ToolDef {
    ToolDef {
        name: "file_read",
        description: "Read one text file from a GitLab project, on the default branch or at a branch, tag or commit. \
                      Returns its content, size, blob id and last commit id. Cut at 25,000 characters; binary files and \
                      files over 1 MB are refused. Use directory_list to find a path first.",
        input_schema: input(vec![
            repo_arg(),
            req("path", string_arg("Path of the file inside the repository, e.g. 'src/index.js'.")),
            opt("ref", string_arg("Branch, tag or commit sha. Default: the default branch.")),
        ]),
        output_schema: output(vec![
            req("path", string()),
            req("blob_id", string()),
            opt("last_commit_id", string()),
            req("size", num()),
            req("text", string()),
            req("truncated", boolean()),
        ]),
        annotations: READ,
    }
}

/// Base64 as `Buffer.from(text, "base64")` reads it: whitespace and line breaks ignored, padding optional.
fn decode_base64(text: &str) -> Vec<u8> {
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let trimmed = cleaned.trim_end_matches('=');
    base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed).unwrap_or_default()
}

pub async fn handle(ctx: &Ctx, args: &Args) -> Result<Outcome, Refused> {
    let repo = parse_repo(arg(args, "repo"))?;
    let file = file_path(arg(args, "path"), "path", false)?;
    let reference = optional_ref(arg(args, "ref"), "ref")?;
    let at_ref = reference.as_ref().map(|r| format!(" at {r}")).unwrap_or_default();

    // `HEAD` is the default branch, which saves a request to find its name.
    let answer = gitlab(
        ctx,
        "GET",
        &files_path(&repo, &file),
        Call::new(format!("read '{}' in {}", file.path, repo.full))
            .query("ref", Some(reference.as_deref().unwrap_or("HEAD")))
            .explain(|status, detail| {
                (status == 404).then(|| {
                    format!(
                        "There is no file '{}' in {}{at_ref} (404){detail}. Check the path with directory_list; a \
                         directory cannot be read as a file. GitLab also answers 404 for a private project the token cannot see.",
                        file.path, repo.full
                    )
                })
            }),
    )
    .await?;

    let body = &answer.body;
    let (Some("base64"), Some(content)) = (at(body, "encoding").as_str(), at(body, "content").as_str()) else {
        return Err(Refused::new("GitLab answered, but the body was not a file this server recognizes."));
    };

    let bytes = decode_base64(content);
    let size = as_int(at(body, "size")).unwrap_or(bytes.len() as i64);
    if size > MAX_READ_BYTES {
        return Err(Refused(format!(
            "`path` '{}' in {} is {size} bytes, over the {MAX_READ_BYTES} this tool reads. Fetch it with git instead.",
            file.path, repo.full
        )));
    }
    if looks_binary(&bytes) {
        return Err(Refused(format!(
            "`path` '{}' is a binary file ({} bytes); this tool reads text.",
            file.path,
            bytes.len()
        )));
    }
    let (text, truncated) = cut_text(&decode_text(&bytes), MAX_FILE_CHARS);

    Ok(Outcome::ok(
        format!(
            "Read {} ({size} bytes) from {}{at_ref}{}",
            file.path,
            repo.full,
            if truncated { format!(", cut at {MAX_FILE_CHARS} characters.") } else { ".".to_string() }
        ),
        Fields::new()
            .set("path", file.path.clone())
            .set("blob_id", or(at(body, "blob_id"), &json!("")).clone())
            .opt("last_commit_id", at(body, "last_commit_id").as_str())
            .set("size", size)
            .set("text", text)
            .set("truncated", truncated),
    ))
}
