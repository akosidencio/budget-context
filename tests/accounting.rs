//! Core accounting and builder contract tests.

use std::time::{Duration, Instant};

use budget_context::{Budget, BudgetBuildError, BudgetError, Remaining, Resource};

fn resource(name: &str) -> Resource {
    Resource::new(name).unwrap()
}

#[test]
fn consume_under_at_and_above_limit() {
    let tokens = resource("tokens");
    let budget = Budget::builder()
        .name("root")
        .limit(tokens.clone(), 10)
        .build()
        .unwrap();

    budget.consume(&tokens, 4).unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(6));
    budget.consume(&tokens, 6).unwrap();
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(0));

    let error = budget.consume(&tokens, 1).unwrap_err();
    assert!(matches!(
        error,
        BudgetError::Exhausted {
            requested: 1,
            remaining: 0,
            ..
        }
    ));
}

#[test]
fn unlimited_resources_are_tracked_and_zero_is_a_noop() {
    let custom = resource("custom");
    let budget = Budget::builder().build().unwrap();

    budget.consume(&custom, 0).unwrap();
    budget.consume(&custom, 7).unwrap();
    assert_eq!(budget.remaining(&custom), Remaining::Unlimited);

    let snapshot = budget.snapshot();
    assert_eq!(snapshot.resources[0].consumed, 7);
}

#[test]
fn duplicate_limits_and_deadlines_are_rejected() {
    let tokens = resource("tokens");
    let duplicate = Budget::builder()
        .limit(tokens.clone(), 1)
        .limit(tokens.clone(), 2)
        .build()
        .unwrap_err();
    assert_eq!(
        duplicate,
        BudgetBuildError::DuplicateLimit { resource: tokens }
    );

    let duplicate_deadline = Budget::builder()
        .deadline_at(Instant::now() + Duration::from_secs(1))
        .deadline_after(Duration::from_secs(1))
        .build()
        .unwrap_err();
    assert_eq!(duplicate_deadline, BudgetBuildError::DuplicateDeadline);
}

#[test]
fn expired_deadline_blocks_new_accounting_but_not_observation() {
    let tokens = resource("tokens");
    let budget = Budget::builder()
        .limit(tokens.clone(), 10)
        .deadline_at(Instant::now())
        .build()
        .unwrap();

    assert_eq!(
        budget.consume(&tokens, 1),
        Err(BudgetError::DeadlineExceeded)
    );
    assert_eq!(budget.remaining(&tokens), Remaining::Limited(10));
    assert_eq!(budget.consume(&tokens, 0), Ok(()));
}

#[test]
fn unlimited_counter_overflow_is_atomic() {
    let units = resource("units");
    let budget = Budget::builder().build().unwrap();
    budget.consume(&units, u64::MAX).unwrap();
    assert_eq!(
        budget.consume(&units, 1),
        Err(BudgetError::Overflow {
            resource: units.clone()
        })
    );
    assert_eq!(budget.snapshot().resources[0].consumed, u64::MAX);
}

#[test]
fn zero_limit_rejects_positive_work() {
    let units = resource("units");
    let budget = Budget::builder().limit(units.clone(), 0).build().unwrap();

    assert_eq!(budget.consume(&units, 0), Ok(()));
    assert!(matches!(
        budget.consume(&units, 1),
        Err(BudgetError::Exhausted {
            requested: 1,
            remaining: 0,
            ..
        })
    ));
}

#[test]
fn reserve_many_overflow_is_deterministic_and_atomic() {
    let alpha = resource("alpha");
    let zulu = resource("zulu");
    let budget = Budget::builder().build().unwrap();

    let error = budget
        .reserve_many([
            (&zulu, u64::MAX),
            (&zulu, 1),
            (&alpha, u64::MAX),
            (&alpha, 1),
        ])
        .unwrap_err();
    assert_eq!(
        error,
        BudgetError::Overflow {
            resource: alpha.clone()
        }
    );
    assert_eq!(budget.remaining(&alpha), Remaining::Unlimited);
    assert!(budget.snapshot().resources.is_empty());
}

#[test]
fn multi_resource_exhaustion_uses_resource_name_order() {
    let alpha = resource("alpha");
    let zulu = resource("zulu");
    let budget = Budget::builder()
        .limit(alpha.clone(), 0)
        .limit(zulu.clone(), 0)
        .build()
        .unwrap();

    let error = budget.reserve_many([(&zulu, 1), (&alpha, 1)]).unwrap_err();
    assert!(matches!(
        error,
        BudgetError::Exhausted { resource, .. } if resource == alpha
    ));
    assert_eq!(budget.remaining(&zulu), Remaining::Limited(0));
}

#[test]
fn deadline_blocks_reservations_but_zero_requests_remain_noops() {
    let units = resource("units");
    let budget = Budget::builder()
        .limit(units.clone(), 10)
        .deadline_at(Instant::now())
        .build()
        .unwrap();

    assert_eq!(
        budget.reserve(&units, 1).unwrap_err(),
        BudgetError::DeadlineExceeded
    );
    assert_eq!(
        budget.reserve_many([(&units, 1)]).unwrap_err(),
        BudgetError::DeadlineExceeded
    );
    budget.reserve(&units, 0).unwrap().commit(0).unwrap();
    budget.reserve_many([(&units, 0)]).unwrap().commit_all();
}
