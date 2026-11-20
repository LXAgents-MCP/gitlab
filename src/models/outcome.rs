//! What a tool returns: a one-sentence summary, the fields its output schema names, in the order the tool
//! wrote them, and whether the call did what was asked.
use serde_json::{Map, Value};

/// The fields of a payload, in order. `opt` leaves a field out when there is nothing to say, as the
/// `...(x ? { key: x } : {})` spreads did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields(pub Map<String, Value>);

impl Fields {
    pub fn new() -> Self {
        Fields(Map::new())
    }

    pub fn set(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.0.insert(key.to_string(), value.into());
        self
    }

    pub fn opt<V: Into<Value>>(self, key: &str, value: Option<V>) -> Self {
        match value {
            Some(value) => self.set(key, value),
            None => self,
        }
    }

    /// Every field of an object, in its order.
    pub fn extend(mut self, other: Fields) -> Self {
        for (key, value) in other.0 {
            self.0.insert(key, value);
        }
        self
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn into_value(self) -> Value {
        Value::Object(self.0)
    }
}

/// A tool's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub summary: String,
    pub fields: Fields,
    pub ok: bool,
}

impl Outcome {
    pub fn ok(summary: impl Into<String>, fields: Fields) -> Self {
        Outcome {
            summary: summary.into(),
            fields,
            ok: true,
        }
    }

    pub fn with_ok(summary: impl Into<String>, fields: Fields, ok: bool) -> Self {
        Outcome {
            summary: summary.into(),
            fields,
            ok,
        }
    }

    /// A field of the payload, or `null`.
    pub fn field(&self, key: &str) -> &Value {
        static NULL: Value = Value::Null;
        self.fields.0.get(key).unwrap_or(&NULL)
    }
}

impl Fields {
    /// A value taken straight from GitLab's answer, left out when it is missing or `null`, as JSON leaves
    /// out a JavaScript `undefined`.
    pub fn raw(self, key: &str, value: &Value) -> Self {
        if value.is_null() {
            self
        } else {
            self.set(key, value.clone())
        }
    }

    /// A value from GitLab's answer, kept only when it is truthy: `...(x ? { key: x } : {})`.
    pub fn truthy(self, key: &str, value: &Value) -> Self {
        if crate::utils::js::truthy(value) {
            self.set(key, value.clone())
        } else {
            self
        }
    }
}
