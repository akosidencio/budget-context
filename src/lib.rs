//! Hierarchical, process-local resource budgets for concurrent Rust task trees.
//!
//! A [`Budget`] accounts for arbitrary integer-valued [`Resource`]s. Child
//! budgets may add tighter limits, while every successful operation is also
//! recorded against every ancestor. [`Reservation`] and [`ReservationSet`]
//! provide cancellation-safe capacity claims which release on drop.
//!
//! The crate is cooperative: code must receive and consult a budget for it to
//! be constrained. It is not a security sandbox, rate limiter, billing ledger,
//! or distributed quota service.

mod budget;
mod error;
mod reservation;
mod resource;
mod snapshot;

#[cfg(feature = "tokio")]
mod tokio_runtime;

pub use budget::{Budget, BudgetBuilder};
pub use error::{BudgetBuildError, BudgetError, ResourceError};
pub use reservation::{Reservation, ReservationSet};
pub use resource::{BudgetId, Resource};
pub use snapshot::{BudgetSnapshot, Remaining, ResourceSnapshot};
