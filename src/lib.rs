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
//!
//! # Example
//!
//! ```
//! use budget_context::{Budget, BudgetError, Remaining, Resource};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let tokens = Resource::new("llm.tokens")?;
//! let task = Budget::builder().limit(tokens.clone(), 10_000).build()?;
//! let researcher = task.child().limit(tokens.clone(), 4_000).build()?;
//!
//! // Reserve before calling out, then reconcile with actual usage.
//! let reservation = researcher.reserve(&tokens, 3_000)?;
//! reservation.commit(1_200)?;
//!
//! assert_eq!(researcher.remaining(&tokens), Remaining::Limited(2_800));
//! assert_eq!(task.remaining(&tokens), Remaining::Limited(8_800));
//! assert!(matches!(
//!     researcher.consume(&tokens, 5_000),
//!     Err(BudgetError::Exhausted { .. })
//! ));
//! # Ok(())
//! # }
//! ```
//!
//! # Feature flags
//!
//! - `tokio`: hierarchical cancellation and deadline-aware `Budget::run`.
//! - `serde`: serialization for resources, snapshots, and errors.
//! - `tracing`: structured accounting events.
//!
//! The [README] covers multi-resource reservations and Tokio usage, and
//! [DESIGN.md] records the exact hierarchy, error-ordering, and
//! reconciliation semantics.
//!
//! [README]: https://github.com/akosidencio/budget-context#readme
//! [DESIGN.md]: https://github.com/akosidencio/budget-context/blob/main/DESIGN.md

#![cfg_attr(docsrs, feature(doc_cfg))]

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

// Keep the README's Rust examples compiled without duplicating it in the
// published API documentation. The README's Tokio example requires all
// features, which is how the documentation test job invokes rustdoc.
#[cfg(all(doctest, feature = "tokio"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
