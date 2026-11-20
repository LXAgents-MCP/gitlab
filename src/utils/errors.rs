//! A request the owner would not want served, or one that cannot succeed.
use std::fmt;

/// A refusal. Carries a message worth reading: what was wrong and what to do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl Refused {
    pub fn new(message: impl Into<String>) -> Self {
        Refused(message.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

/// Refuse with a formatted message: `refuse!("... {x} ...")` is `Err(Refused(format!(...)))`.
#[macro_export]
macro_rules! refuse {
    ($($arg:tt)*) => {
        return Err($crate::utils::errors::Refused(format!($($arg)*)))
    };
}
