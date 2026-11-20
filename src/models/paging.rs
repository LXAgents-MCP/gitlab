//! The paging arguments every list tool takes, and the paging fields it returns.
use std::collections::BTreeMap;

use serde_json::Value;

use super::args::{Args, IntRange, arg, optional_int};
use super::outcome::Fields;
use crate::config::config::{DEFAULT_PER_PAGE, MAX_PER_PAGE};
use crate::utils::errors::Refused;

/// The page a list tool asks GitLab for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageQuery {
    pub page: i64,
    pub per_page: i64,
}

impl PageQuery {
    /// As query parameters, after the tool's own.
    pub fn query(&self) -> [(&'static str, Option<String>); 2] {
        [
            ("page", Some(self.page.to_string())),
            ("per_page", Some(self.per_page.to_string())),
        ]
    }
}

/// Read `page` and `per_page` from a tool's arguments.
pub fn page_query(args: &Args) -> Result<PageQuery, Refused> {
    Ok(PageQuery {
        page: optional_int(arg(args, "page"), "page", IntRange::min(1))?.unwrap_or(1),
        per_page: optional_int(
            arg(args, "per_page"),
            "per_page",
            IntRange::between(1, MAX_PER_PAGE),
        )?
        .unwrap_or(DEFAULT_PER_PAGE),
    })
}

/// The paging fields of a list result, added to `fields`: the page asked for, and whether GitLab has
/// another. `headers` are the response headers, lowercased.
pub fn paging(fields: Fields, query: PageQuery, headers: &BTreeMap<String, String>) -> Fields {
    // Most answers carry `x-next-page`; the commits API, which sends no totals, is read by its link
    // header too.
    let next = headers.get("x-next-page").is_some_and(|v| !v.is_empty())
        || headers
            .get("link")
            .is_some_and(|v| v.contains("rel=\"next\""));
    fields
        .set("page", Value::from(query.page))
        .set("per_page", Value::from(query.per_page))
        .set("has_more", Value::Bool(next))
}
