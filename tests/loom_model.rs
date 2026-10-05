//! Loom model of the lineage locking protocol, run against the real crate.
//!
//! Run with:
//!
//! ```text
//! RUSTFLAGS="--cfg budget_context_loom" cargo test --release --test loom_model
//! ```

#![cfg(budget_context_loom)]

use budget_context::{Budget, Remaining, Resource};
use loom::thread;

fn tree(limit: u64) -> (Resource, Budget, Budget, Budget) {
    let units = Resource::new("units").unwrap();
    let root = Budget::builder()
        .limit(units.clone(), limit)
        .build()
        .unwrap();
    let left = root.child().build().unwrap();
    let right = root.child().build().unwrap();
    (units, root, left, right)
}

fn assert_within_limit(budget: &Budget, units: &Resource, limit: u64) {
    let snapshot = budget.snapshot();
    let usage = snapshot
        .resources
        .iter()
        .find(|resource| &resource.resource == units)
        .unwrap();
    assert!(usage.consumed + usage.reserved <= limit);
}

#[test]
fn sibling_reservations_never_oversubscribe_the_root() {
    loom::model(|| {
        let (units, root, left, right) = tree(10);
        let handles: Vec<_> = [left, right]
            .into_iter()
            .map(|child| {
                let units = units.clone();
                thread::spawn(move || {
                    child
                        .reserve(&units, 8)
                        .map(|reservation| reservation.commit(8))
                })
            })
            .collect();
        let admitted = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(Result::is_ok)
            .count();

        assert_eq!(admitted, 1);
        assert_eq!(root.remaining(&units), Remaining::Limited(2));
        assert_within_limit(&root, &units, 10);
    });
}

#[test]
fn release_racing_with_a_sibling_consume_preserves_the_limit() {
    loom::model(|| {
        let (units, root, left, right) = tree(10);
        let reservation = left.reserve(&units, 8).unwrap();

        let release = thread::spawn(move || reservation.release());
        let consume_units = units.clone();
        let consume = thread::spawn(move || right.consume(&consume_units, 7).is_ok());

        release.join().unwrap();
        let consumed = consume.join().unwrap();

        let expected = if consumed { 3 } else { 10 };
        assert_eq!(root.remaining(&units), Remaining::Limited(expected));
        assert_within_limit(&root, &units, 10);
    });
}

#[test]
fn commit_racing_with_a_sibling_consume_preserves_the_limit() {
    loom::model(|| {
        let (units, root, left, right) = tree(10);
        let reservation = left.reserve(&units, 8).unwrap();

        let commit = thread::spawn(move || reservation.commit(3).unwrap());
        let consume_units = units.clone();
        let consume = thread::spawn(move || right.consume(&consume_units, 7).is_ok());

        commit.join().unwrap();
        let consumed = consume.join().unwrap();

        let expected = if consumed { 0 } else { 7 };
        assert_eq!(root.remaining(&units), Remaining::Limited(expected));
        assert_within_limit(&root, &units, 10);
    });
}

#[test]
fn snapshots_observe_a_consistent_lineage() {
    loom::model(|| {
        let (units, root, left, _) = tree(10);
        let reserve_units = units.clone();
        let reserve = thread::spawn(move || left.reserve(&reserve_units, 4).unwrap().commit(4));

        let observed = root.remaining(&units);
        assert!(observed == Remaining::Limited(10) || observed == Remaining::Limited(6));

        reserve.join().unwrap().unwrap();
        assert_eq!(root.remaining(&units), Remaining::Limited(6));
    });
}
