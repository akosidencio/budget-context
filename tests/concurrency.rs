//! Multi-threaded accounting stress tests.

use std::sync::{Arc, Barrier};
use std::thread;

use budget_context::{Budget, Resource};

#[test]
fn concurrent_consumers_never_oversubscribe() {
    let units = Resource::new("units").unwrap();
    let budget = Arc::new(
        Budget::builder()
            .limit(units.clone(), 1_000)
            .build()
            .unwrap(),
    );

    let handles: Vec<_> = (0..32)
        .map(|_| {
            let budget = budget.clone();
            let units = units.clone();
            thread::spawn(move || {
                let mut successes = 0;
                while budget.consume(&units, 1).is_ok() {
                    successes += 1;
                }
                successes
            })
        })
        .collect();

    let successes: u64 = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .sum();
    assert_eq!(successes, 1_000);

    let snapshot = budget.snapshot();
    assert_eq!(snapshot.resources[0].consumed, 1_000);
    assert_eq!(snapshot.resources[0].reserved, 0);
}

#[test]
fn exactly_one_competing_large_reservation_wins() {
    let units = Resource::new("units").unwrap();
    let budget = Arc::new(Budget::builder().limit(units.clone(), 10).build().unwrap());

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let budget = budget.clone();
            let units = units.clone();
            thread::spawn(move || budget.reserve(&units, 8))
        })
        .collect();
    let reservations: Vec<_> = handles
        .into_iter()
        .filter_map(|handle| handle.join().unwrap().ok())
        .collect();
    assert_eq!(reservations.len(), 1);
}

#[test]
fn multi_resource_requests_remain_all_or_nothing_under_contention() {
    let tokens = Resource::new("tokens").unwrap();
    let requests = Resource::new("requests").unwrap();
    let budget = Arc::new(
        Budget::builder()
            .limit(tokens.clone(), 10)
            .limit(requests.clone(), 1)
            .build()
            .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(3));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let budget = budget.clone();
            let tokens = tokens.clone();
            let requests = requests.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                budget.reserve_many([(&tokens, 8), (&requests, 1)])
            })
        })
        .collect();
    barrier.wait();

    let permits: Vec<_> = handles
        .into_iter()
        .filter_map(|handle| handle.join().unwrap().ok())
        .collect();
    assert_eq!(permits.len(), 1);
    let snapshot = budget.snapshot();
    let token_usage = snapshot
        .resources
        .iter()
        .find(|entry| entry.resource == tokens)
        .unwrap();
    let request_usage = snapshot
        .resources
        .iter()
        .find(|entry| entry.resource == requests)
        .unwrap();
    assert_eq!(token_usage.reserved, 8);
    assert_eq!(request_usage.reserved, 1);
}
