//! Serde snapshot format tests.

#![cfg(feature = "serde")]

use budget_context::{Budget, BudgetError, BudgetSnapshot, Remaining, Resource};

#[test]
fn snapshots_serialize_without_serializing_live_budgets() {
    let units = Resource::new("units").unwrap();
    let budget = Budget::builder()
        .name("root")
        .limit(units.clone(), 10)
        .build()
        .unwrap();
    budget.consume(&units, 3).unwrap();

    let json = serde_json::to_value(budget.snapshot()).unwrap();
    assert_eq!(json["name"], "root");
    assert_eq!(json["resources"][0]["consumed"], 3);
}

#[test]
fn public_observation_and_error_types_round_trip() {
    let resource = Resource::new("llm.tokens").unwrap();
    let resource_json = serde_json::to_string(&resource).unwrap();
    assert_eq!(
        serde_json::from_str::<Resource>(&resource_json).unwrap(),
        resource
    );

    let remaining = Remaining::Limited(42);
    let remaining_json = serde_json::to_string(&remaining).unwrap();
    assert_eq!(
        serde_json::from_str::<Remaining>(&remaining_json).unwrap(),
        remaining
    );

    let error = BudgetError::ReservationExceeded {
        resource: resource.clone(),
        reserved: 8,
        actual: 10,
        unaccounted: 2,
    };
    let error_json = serde_json::to_string(&error).unwrap();
    assert_eq!(
        serde_json::from_str::<BudgetError>(&error_json).unwrap(),
        error
    );

    let budget = Budget::builder()
        .limit(resource.clone(), 100)
        .build()
        .unwrap();
    budget.consume(&resource, 7).unwrap();
    let snapshot = budget.snapshot();
    let snapshot_json = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_str::<BudgetSnapshot>(&snapshot_json).unwrap(),
        snapshot
    );
}
