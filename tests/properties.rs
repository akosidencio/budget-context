//! Property tests for the primary accounting invariant.

use budget_context::{Budget, Resource};
use proptest::prelude::*;

proptest! {
    #[test]
    fn arbitrary_consumption_never_exceeds_limit(amounts in prop::collection::vec(0_u16..200, 0..500)) {
        let units = Resource::new("units").unwrap();
        let budget = Budget::builder()
            .limit(units.clone(), 10_000)
            .build()
            .unwrap();

        for amount in amounts {
            let _ = budget.consume(&units, u64::from(amount));
            let snapshot = budget.snapshot();
            let usage = &snapshot.resources[0];
            prop_assert!(usage.consumed + usage.reserved <= 10_000);
        }
    }

    #[test]
    fn reservation_reconciliation_preserves_invariant(
        reserved in 0_u16..500,
        actual in 0_u16..700,
    ) {
        let units = Resource::new("units").unwrap();
        let budget = Budget::builder()
            .limit(units.clone(), 500)
            .build()
            .unwrap();
        let reservation = budget.reserve(&units, u64::from(reserved)).unwrap();
        let _ = reservation.commit(u64::from(actual));
        let usage = &budget.snapshot().resources[0];
        prop_assert!(usage.consumed + usage.reserved <= 500);
    }

    #[test]
    fn arbitrary_mixed_operation_sequences_preserve_invariants(
        actions in prop::collection::vec((0_u8..4, 0_u16..200, 0_u16..250), 0..300),
    ) {
        let units = Resource::new("units").unwrap();
        let budget = Budget::builder()
            .limit(units.clone(), 1_000)
            .build()
            .unwrap();

        for (kind, requested, actual) in actions {
            let requested = u64::from(requested);
            let actual = u64::from(actual);
            match kind {
                0 => {
                    let _ = budget.consume(&units, requested);
                }
                1 => {
                    if let Ok(reservation) = budget.reserve(&units, requested) {
                        drop(reservation);
                    }
                }
                2 => {
                    if let Ok(reservation) = budget.reserve(&units, requested) {
                        let _ = reservation.commit(actual);
                    }
                }
                _ => {
                    if let Ok(reservation) = budget.reserve(&units, requested) {
                        reservation.release();
                    }
                }
            }

            let usage = &budget.snapshot().resources[0];
            prop_assert!(usage.consumed + usage.reserved <= 1_000);
        }
    }
}
