//! Single-resource and multi-resource reservation tests.

use budget_context::{Budget, BudgetError, Remaining, Resource};

fn setup() -> (Budget, Resource, Resource) {
    let tokens = Resource::new("tokens").unwrap();
    let requests = Resource::new("requests").unwrap();
    let budget = Budget::builder()
        .limit(tokens.clone(), 100)
        .limit(requests.clone(), 10)
        .build()
        .unwrap();
    (budget, tokens, requests)
}

#[test]
fn reservation_commit_drop_and_explicit_release() {
    let (budget, tokens, _) = setup();

    let reservation = budget.reserve(&tokens, 40).unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(60));
    drop(reservation);
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(100));

    budget.reserve(&tokens, 40).unwrap().commit(13).unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(87));

    budget.reserve(&tokens, 20).unwrap().release();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(87));
}

#[test]
fn overage_fails_closed() {
    let (budget, tokens, _) = setup();

    let error = budget.reserve(&tokens, 40).unwrap().commit(45).unwrap_err();
    assert_eq!(
        error,
        BudgetError::ReservationExceeded {
            resource: tokens.clone(),
            reserved: 40,
            actual: 45,
            unaccounted: 5,
        }
    );
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(60));
    let snapshot = budget.snapshot();
    let token_usage = snapshot
        .resources
        .iter()
        .find(|entry| entry.resource == tokens)
        .unwrap();
    assert_eq!(token_usage.consumed, 40);
    assert_eq!(token_usage.reserved, 0);
}

#[test]
fn reserve_many_is_atomic_and_canonicalizes_duplicates() {
    let (budget, tokens, requests) = setup();

    let permit = budget
        .reserve_many([(&tokens, 20), (&requests, 1), (&tokens, 5)])
        .unwrap();
    assert_eq!(permit.amounts().get(&tokens), Some(&25));
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(75));

    permit
        .commit([(&tokens, 7), (&tokens, 3), (&requests, 1)])
        .unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(90));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(9));

    let result = budget.reserve_many([(&tokens, 90), (&requests, 10)]);
    assert!(result.is_err());
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(90));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(9));
}

#[test]
fn malformed_multi_commit_consumes_all_reservations() {
    let (budget, tokens, requests) = setup();
    let unknown = Resource::new("unknown").unwrap();
    let permit = budget
        .reserve_many([(&tokens, 20), (&requests, 2)])
        .unwrap();

    let error = permit.commit([(&unknown, 1)]).unwrap_err();
    assert_eq!(
        error,
        BudgetError::UnknownReservationResource { resource: unknown }
    );
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(80));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(8));
}

#[test]
fn zero_reservation_handles_reported_overage_without_panicking() {
    let (budget, tokens, _) = setup();
    budget.reserve(&tokens, 0).unwrap().commit(0).unwrap();
    assert!(matches!(
        budget.reserve(&tokens, 0).unwrap().commit(1),
        Err(BudgetError::ReservationExceeded { reserved: 0, .. })
    ));
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(100));
}

#[test]
fn malformed_resource_selection_is_deterministic() {
    let (budget, tokens, _) = setup();
    let alpha = Resource::new("alpha").unwrap();
    let zulu = Resource::new("zulu").unwrap();
    let permit = budget.reserve_many([(&tokens, 20)]).unwrap();

    let error = permit.commit([(&zulu, 1), (&alpha, 1)]).unwrap_err();
    assert_eq!(
        error,
        BudgetError::UnknownReservationResource { resource: alpha }
    );
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(80));
}

#[test]
fn reservation_accessors_report_the_claim() {
    let (budget, tokens, _) = setup();
    let reservation = budget.reserve(&tokens, 17).unwrap();
    assert_eq!(reservation.resource(), &tokens);
    assert_eq!(reservation.amount(), 17);
}

#[test]
fn empty_and_zero_only_reservation_sets_are_inert() {
    let (budget, tokens, _) = setup();

    let empty = budget.reserve_many([]).unwrap();
    assert!(empty.amounts().is_empty());
    empty.commit([]).unwrap();

    budget.reserve_many([]).unwrap().commit_all();
    budget.reserve_many([]).unwrap().release();
    drop(budget.reserve_many([(&tokens, 0)]).unwrap());
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(100));
}

#[test]
fn reservation_set_drop_and_release_restore_every_resource() {
    let (budget, tokens, requests) = setup();

    drop(
        budget
            .reserve_many([(&tokens, 20), (&requests, 2)])
            .unwrap(),
    );
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(100));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(10));

    budget
        .reserve_many([(&tokens, 20), (&requests, 2)])
        .unwrap()
        .release();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(100));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(10));
}

#[test]
fn commit_all_consumes_every_resource_and_omitted_actuals_release() {
    let (budget, tokens, requests) = setup();

    budget
        .reserve_many([(&tokens, 20), (&requests, 2)])
        .unwrap()
        .commit_all();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(80));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(8));

    budget
        .reserve_many([(&tokens, 10), (&requests, 1)])
        .unwrap()
        .commit([(&requests, 1)])
        .unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(80));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(7));
}

#[test]
fn multi_reservation_overage_reconciles_all_and_reports_first_resource() {
    let (budget, tokens, requests) = setup();
    let error = budget
        .reserve_many([(&tokens, 20), (&requests, 2)])
        .unwrap()
        .commit([(&tokens, 21), (&requests, 3)])
        .unwrap_err();

    assert_eq!(
        error,
        BudgetError::ReservationExceeded {
            resource: requests.clone(),
            reserved: 2,
            actual: 3,
            unaccounted: 1,
        }
    );
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(80));
    assert_eq!(budget.remaining(&requests), Remaining::Limited(8));
}

#[test]
fn actual_entry_overflow_fails_closed() {
    let units = Resource::new("units").unwrap();
    let budget = Budget::builder()
        .limit(units.clone(), u64::MAX)
        .build()
        .unwrap();
    let permit = budget.reserve_many([(&units, u64::MAX)]).unwrap();

    let error = permit
        .commit([(&units, u64::MAX), (&units, 1)])
        .unwrap_err();
    assert_eq!(
        error,
        BudgetError::Overflow {
            resource: units.clone()
        }
    );
    assert_eq!(budget.remaining(&units), Remaining::Limited(0));
}
