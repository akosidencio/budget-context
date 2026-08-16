//! Tokio cancellation and deadline integration tests.

#![cfg(feature = "tokio")]

use std::time::Duration;

use budget_context::{Budget, BudgetError, Remaining, Resource};

#[tokio::test]
async fn parent_cancellation_propagates_down_but_not_up() {
    let root = Budget::builder().build().unwrap();
    let left = root.child().build().unwrap();
    let right = root.child().build().unwrap();
    left.cancel();
    assert!(left.is_cancelled());
    assert!(!root.is_cancelled());
    assert!(!right.is_cancelled());

    root.cancel();
    assert!(root.is_cancelled());
    assert!(right.is_cancelled());
    assert!(root.child().build().unwrap().is_cancelled());
}

#[tokio::test]
async fn run_returns_completion_cancellation_and_deadline() {
    let complete = Budget::builder().build().unwrap();
    assert_eq!(complete.run(async { 42 }).await, Ok(42));

    let cancelled = Budget::builder().build().unwrap();
    cancelled.cancel();
    assert_eq!(
        cancelled.run(std::future::pending::<()>()).await,
        Err(BudgetError::Cancelled)
    );

    let expired = Budget::builder()
        .deadline_after(Duration::ZERO)
        .build()
        .unwrap();
    assert_eq!(
        expired.run(std::future::pending::<()>()).await,
        Err(BudgetError::DeadlineExceeded)
    );
}

#[tokio::test]
async fn cancellation_blocks_new_work_but_reservations_can_reconcile() {
    let tokens = Resource::new("tokens").unwrap();
    let budget = Budget::builder()
        .limit(tokens.clone(), 100)
        .build()
        .unwrap();
    let reservation = budget.reserve(&tokens, 40).unwrap();
    budget.cancel();

    assert_eq!(budget.consume(&tokens, 1), Err(BudgetError::Cancelled));
    reservation.commit(10).unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(90));
}

#[tokio::test]
async fn run_observes_cancellation_after_execution_starts() {
    let budget = Budget::builder().build().unwrap();
    let canceller = budget.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        canceller.cancel();
    });

    assert_eq!(
        budget.run(std::future::pending::<()>()).await,
        Err(BudgetError::Cancelled)
    );
}

#[tokio::test]
async fn run_completes_with_an_active_deadline_and_times_out_after_start() {
    let complete = Budget::builder()
        .deadline_after(Duration::from_secs(1))
        .build()
        .unwrap();
    assert_eq!(complete.run(async { 42 }).await, Ok(42));

    let timeout = Budget::builder()
        .deadline_after(Duration::from_millis(10))
        .build()
        .unwrap();
    assert_eq!(
        timeout.run(std::future::pending::<()>()).await,
        Err(BudgetError::DeadlineExceeded)
    );
}

#[tokio::test]
async fn cancellation_blocks_positive_reservations_and_marks_snapshots() {
    let units = Resource::new("units").unwrap();
    let budget = Budget::builder().limit(units.clone(), 10).build().unwrap();
    budget.cancel();

    assert_eq!(
        budget.reserve(&units, 1).unwrap_err(),
        BudgetError::Cancelled
    );
    assert_eq!(
        budget.reserve_many([(&units, 1)]).unwrap_err(),
        BudgetError::Cancelled
    );
    assert!(budget.snapshot().cancelled);

    budget.reserve(&units, 0).unwrap().commit(0).unwrap();
    budget.reserve_many([(&units, 0)]).unwrap().commit_all();
}
