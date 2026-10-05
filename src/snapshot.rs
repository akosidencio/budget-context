use std::sync::Arc;
use std::time::Duration;

use crate::{BudgetId, Resource};

/// Effective remaining capacity for a resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Remaining {
    /// No node in the lineage limits the resource.
    Unlimited,
    /// The minimum remaining capacity across limited lineage nodes.
    Limited(u64),
}

/// A consistent read-only view of one resource at a budget node.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ResourceSnapshot {
    /// The resource category.
    pub resource: Resource,
    /// The limit configured directly on the observed node.
    pub local_limit: Option<u64>,
    /// Consumed usage for the complete subtree rooted at the observed node.
    pub consumed: u64,
    /// Reserved usage for the complete subtree rooted at the observed node.
    pub reserved: u64,
    /// Effective capacity after applying every ancestor limit.
    pub effective_remaining: Remaining,
}

/// A consistent read-only view of a budget node.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct BudgetSnapshot {
    /// Process-local node identifier.
    pub id: BudgetId,
    /// Optional diagnostic name.
    pub name: Option<Arc<str>>,
    /// Resources limited in the lineage or observed in this subtree.
    pub resources: Vec<ResourceSnapshot>,
    /// Saturating duration until the effective deadline.
    pub deadline_remaining: Option<Duration>,
    /// Whether cancellation has been requested for this node.
    ///
    /// Always `false` without the `tokio` feature.
    pub cancelled: bool,
}
