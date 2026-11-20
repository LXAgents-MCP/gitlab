//! Who this server is. The version is the package's, compiled in, so the binary needs no file beside it.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SERVER_NAME: &str = "lxagents-gitlab";
