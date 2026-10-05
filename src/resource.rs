use std::fmt;
use std::sync::Arc;

use crate::ResourceError;

/// An application-defined resource category.
///
/// Names are non-empty, case-sensitive UTF-8 strings. The crate does not
/// normalize them or attach meaning to namespace separators.
///
/// Budgets retain an entry for every distinct resource charged against them,
/// so resources should come from a fixed vocabulary rather than unbounded
/// input.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

// Deserialization goes through `Resource::new` so that serialized input cannot
// produce an empty name. The private mirror keeps the derived wire format.
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Resource {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(rename = "Resource")]
        struct Unvalidated(Arc<str>);

        let Unvalidated(name) = Unvalidated::deserialize(deserializer)?;
        Self::new(name).map_err(serde::de::Error::custom)
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
