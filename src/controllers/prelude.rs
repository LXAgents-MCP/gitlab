//! What every tool module uses, in one import.
pub use serde_json::{Value, json};

pub use super::ToolDef;
pub use crate::config::config::*;
pub use crate::models::address::{
    FilePath, Repo, file_path, optional_ref, parse_repo, ref_name, ref_path,
};
pub use crate::models::args::*;
pub use crate::models::outcome::{Fields, Outcome};
pub use crate::models::paging::{page_query, paging};
pub use crate::models::schemas::*;
pub use crate::models::shapes::expect_body;
pub use crate::services::api::{Call, branch_or_default, gitlab};
pub use crate::services::context::Ctx;
pub use crate::utils::errors::Refused;
pub use crate::utils::js::{as_int, at, is_int, js_string, list, nonempty_str, or, truthy};
pub use crate::utils::text::{first_line, short};

/// "branch" or "branches": a count with the right noun.
pub fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}
