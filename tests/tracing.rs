//! Structured tracing integration smoke tests.

#![cfg(feature = "tracing")]

use budget_context::{Budget, Resource};
use tracing::Level;

#[test]
fn accounting_and_cancellation_events_evaluate_all_fields() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_test_writer()
        .without_time()
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let tokens = Resource::new("tokens").unwrap();
        let requests = Resource::new("requests").unwrap();
        let budget = Budget::builder()
            .limit(tokens.clone(), 100)
            .limit(requests.clone(), 10)
            .build()
            .unwrap();

        budget.consume(&tokens, 1).unwrap();
        budget.reserve(&tokens, 5).unwrap().commit(3).unwrap();
        budget
            .reserve_many([(&tokens, 5), (&requests, 1)])
            .unwrap()
            .release();

        #[cfg(feature = "tokio")]
        budget.cancel();
    });
}
