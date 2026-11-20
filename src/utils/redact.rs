//! Keeping a secret out of text that is about to be shown or logged.

/// Remove a secret from text that is about to be shown.
pub fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_string()
    } else {
        text.replace(secret, "***")
    }
}
