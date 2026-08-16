use std::fmt;
use std::sync::Arc;

use crate::ResourceError;

/// An application-defined resource category.
///
/// Names are non-empty, case-sensitive UTF-8 strings. The crate does not
/// normalize them or attach meaning to namespace separators.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Resource(Arc<str>);

impl Resource {
    /// Creates a resource, rejecting an empty name.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceError::EmptyName`] when `name` is empty.
    pub fn new(name: impl Into<Arc<str>>) -> Result<Self, ResourceError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ResourceError::EmptyName);
        }
        Ok(Self(name))
    }

    /// Returns the resource name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A process-local identifier for a budget node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BudgetId(pub(crate) u64);

impl BudgetId {
    /// Returns the numeric process-local identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for BudgetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
