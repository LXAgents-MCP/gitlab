//! Settings and constants. The bottom layer: it imports nothing from the rest of `src/`.
#[allow(clippy::module_inception)] // config/config mirrors the layout every LXAgents server shares.
pub mod config;
pub mod http_settings;
pub mod version;
