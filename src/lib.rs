//! `lxagents-gitlab`: an MCP server that gives an LLM access to GitLab through its REST API, over stdio or
//! over HTTP behind an access token.
//!
//! The layers import downward only: `routes` → `controllers` → `services` → `models` → `utils` →
//! `config`.
pub mod config;
pub mod controllers;
pub mod models;
pub mod routes;
pub mod services;
pub mod utils;
