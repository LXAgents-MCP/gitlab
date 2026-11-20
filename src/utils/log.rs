//! Diagnostics go to stderr, always.
//!
//! stdout is the protocol channel. One stray write corrupts the stream and the client reports a JSON
//! parse error that points at the payload rather than at the logging, which is the single most common way
//! a stdio MCP server fails. Nothing in `src/` writes to stdout; the SDK's transport is the only thing
//! that does.
use std::io::Write;

pub fn log(message: &str) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{message}");
}
