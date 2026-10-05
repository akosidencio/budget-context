use std::sync::Arc;

use crate::{BudgetId, Resource};

/// Error returned when constructing a resource.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ResourceError {
    /// Resource names must contain at least one byte.
    #[error("resource name cannot be empty")]
    EmptyName,
}

/// Error returned when building a budget node.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BudgetBuildError {
    /// A resource limit was configured more than once on the same node.
    #[error("duplicate limit for resource {resource}")]
    DuplicateLimit {
        /// The duplicated resource.
        resource: Resource,
    },

    /// A relative deadline could not be represented by the platform clock.
    #[error("deadline is outside the representable clock range")]
    DeadlineOverflow,

    /// More than one deadline was configured on the builder.
    #[error("deadline configured more than once")]
    DuplicateDeadline,

    /// The process-local budget identifier space was exhausted.
    #[error("process-local budget identifier space exhausted")]
    BudgetIdExhausted,
}

/// Error returned by accounting and execution operations.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BudgetError {
    /// A limit in the budget lineage rejected an operation.
    #[error(
        "resource {resource} exhausted at budget {scope}: requested {requested}, remaining {remaining}"
    )]
    Exhausted {
        /// The exhausted resource.
        resource: Resource,
        /// The amount requested by the operation.
        requested: u64,
        /// Capacity remaining at the limiting node.
        remaining: u64,
        /// Identifier of the first limiting node in root-to-leaf order.
        scope: BudgetId,
        /// Optional name of the limiting node.
        scope_name: Option<Arc<str>>,
    },

    /// A reservation was reconciled with more usage than it held.
    ///
    /// The full reservation is retained as consumed. `unaccounted` reports
    /// usage which could not be admitted without violating a budget limit.
    #[error("resource {resource} used {actual}, exceeding reservation {reserved} by {unaccounted}")]
    ReservationExceeded {
        /// The resource whose actual usage exceeded its reservation.
        resource: Resource,
        /// Reserved capacity converted to consumed capacity.
        reserved: u64,
        /// Actual usage reported by the caller.
        actual: u64,
        /// Actual usage beyond the reservation.
        unaccounted: u64,
    },

    /// Actual usage named a resource absent from the reservation set.
    ///
    /// To fail closed, all reservations in the set are converted to consumed
    /// capacity before this error is returned.
    #[error("resource {resource} was not part of the reservation set")]
    UnknownReservationResource {
        /// The unexpected resource.
        resource: Resource,
    },

    /// Checked resource arithmetic overflowed.
    #[error("resource arithmetic overflow for {resource}")]
    Overflow {
        /// The overflowing resource.
        resource: Resource,
    },

    /// The effective budget deadline elapsed before an operation began.
    #[error("budget deadline exceeded")]
    DeadlineExceeded,

    /// The budget was cancelled.
    ///
    /// Only produced with the `tokio` feature, but always present so that
    /// enabling a feature never changes this type.
    #[error("budget cancelled")]
    Cancelled,
}
