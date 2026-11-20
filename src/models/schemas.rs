//! Schema fragments and annotations shared by the tools, so a repository, a page or a hint is described
//! once and every tool says it the same way.
//!
//! The JSON written here is what the TypeScript server published from its zod schemas, kept exactly so a
//! client sees the same surface: an input schema has no `additionalProperties`, a whole number is an
//! `integer` bounded by the safe-integer range unless it says otherwise, and an output schema closes
//! every object with `additionalProperties: false` and calls every number a `number`.
//! `test/fixtures/tools-list.json` pins the result.
use serde_json::{Map, Value, json};

use super::args::MAX_SAFE_INTEGER;

const DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";

/// One property of an object schema: its name, its schema, and whether it is required.
pub type Prop = (&'static str, Value, bool);

/// A required property.
pub fn req(name: &'static str, schema: Value) -> Prop {
    (name, schema, true)
}

/// An optional property.
pub fn opt(name: &'static str, schema: Value) -> Prop {
    (name, schema, false)
}

fn object(props: Vec<Prop>, closed: bool) -> Map<String, Value> {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (name, schema, is_required) in props {
        if is_required {
            required.push(Value::from(name));
        }
        properties.insert(name.to_string(), schema);
    }
    let mut object = Map::new();
    object.insert("type".into(), Value::from("object"));
    object.insert("properties".into(), Value::Object(properties));
    if !required.is_empty() {
        object.insert("required".into(), Value::Array(required));
    }
    if closed {
        object.insert("additionalProperties".into(), Value::Bool(false));
    }
    object
}

fn with_schema(mut object: Map<String, Value>) -> Value {
    object.insert("$schema".into(), Value::from(DRAFT_07));
    Value::Object(object)
}

/// A tool's input schema.
pub fn input(props: Vec<Prop>) -> Value {
    with_schema(object(props, false))
}

/// A tool's output schema.
pub fn output(props: Vec<Prop>) -> Value {
    with_schema(object(props, true))
}

/* ------------------------------------------------------------------ input fragments */

fn described(mut schema: Value, description: &str) -> Value {
    schema["description"] = Value::from(description);
    schema
}

pub fn string_arg(description: &str) -> Value {
    described(json!({ "type": "string" }), description)
}

pub fn bool_arg(description: &str) -> Value {
    described(json!({ "type": "boolean" }), description)
}

pub fn int_arg(description: &str) -> Value {
    int_range_arg(description, -MAX_SAFE_INTEGER, MAX_SAFE_INTEGER)
}

pub fn int_range_arg(description: &str, minimum: i64, maximum: i64) -> Value {
    described(
        json!({ "type": "integer", "minimum": minimum, "maximum": maximum }),
        description,
    )
}

pub fn string_list_arg(description: &str) -> Value {
    described(
        json!({ "type": "array", "items": { "type": "string" } }),
        description,
    )
}

pub fn enum_arg(values: &[&str], description: &str) -> Value {
    described(json!({ "type": "string", "enum": values }), description)
}

/// The `repo` argument every tool takes except `repo_create` and `repo_search`. A repository is a project
/// here.
pub fn repo_arg() -> Prop {
    req(
        "repo",
        string_arg(
            "Project: its full path, such as 'lxagents-mcp/gitlab' or 'group/subgroup/project', or its numeric id.",
        ),
    )
}

/// Paging arguments for a list.
pub fn paging_args() -> Vec<Prop> {
    vec![
        opt(
            "page",
            int_range_arg("Page number, from 1. Default 1.", 1, MAX_SAFE_INTEGER),
        ),
        opt(
            "per_page",
            int_range_arg("Results per page, 1 to 100. Default 30.", 1, 100),
        ),
    ]
}

/* ------------------------------------------------------------------ output fragments */

pub fn num() -> Value {
    json!({ "type": "number" })
}

pub fn string() -> Value {
    json!({ "type": "string" })
}

pub fn boolean() -> Value {
    json!({ "type": "boolean" })
}

pub fn strings() -> Value {
    array(string())
}

pub fn array(items: Value) -> Value {
    json!({ "type": "array", "items": items })
}

/// A nested object of an output schema.
pub fn obj(props: Vec<Prop>) -> Value {
    Value::Object(object(props, true))
}

/// Paging fields of a list result.
pub fn paging_out() -> Vec<Prop> {
    vec![
        req("page", num()),
        req("per_page", num()),
        req("has_more", boolean()),
    ]
}

/// A changed file, as commits, comparisons and merge requests list one.
pub fn file_summary() -> Value {
    obj(vec![
        req("filename", string()),
        req("status", string()),
        req("additions", num()),
        req("deletions", num()),
    ])
}

/* ------------------------------------------------------------------ annotations */

/// What a tool says it does, so a client can decide whether to ask first. They are honest or they are
/// worse than none: a read that claims to be read-only and writes is trusted exactly because it said so.
/// `open_world` is true everywhere - every tool talks to GitLab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hints {
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
}

impl Hints {
    pub fn to_json(self) -> Value {
        json!({
            "readOnlyHint": self.read_only,
            "destructiveHint": self.destructive,
            "idempotentHint": self.idempotent,
            "openWorldHint": self.open_world,
        })
    }
}

/// Reads and changes nothing.
pub const READ: Hints = Hints {
    read_only: true,
    destructive: false,
    idempotent: true,
    open_world: true,
};

/// Creates something new; repeating the call creates it again or fails.
pub const WRITE: Hints = Hints {
    read_only: false,
    destructive: false,
    idempotent: false,
    open_world: true,
};

/// Changes something that exists; repeating the call with the same arguments changes nothing more.
pub const UPDATE: Hints = Hints {
    read_only: false,
    destructive: false,
    idempotent: true,
    open_world: true,
};

/// Deletes, overwrites or merges, which cannot be taken back from here.
pub const DESTROY: Hints = Hints {
    read_only: false,
    destructive: true,
    idempotent: true,
    open_world: true,
};
