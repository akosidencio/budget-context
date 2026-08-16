//! Public value-type, identifier, display, and error-contract tests.

use budget_context::{Budget, BudgetError, Resource, ResourceError};

#[test]
fn resource_names_are_nonempty_exact_and_case_sensitive() {
    assert_eq!(Resource::new(""), Err(ResourceError::EmptyName));

    let lower = Resource::new("llm.tokens").unwrap();
    let upper = Resource::new("LLM.Tokens").unwrap();
    assert_ne!(lower, upper);
    assert_eq!(lower.as_str(), "llm.tokens");
    assert_eq!(lower.to_string(), "llm.tokens");
}

#[test]
fn budget_ids_are_unique_numeric_and_displayable() {
    let first = Budget::builder().build().unwrap().id();
    let second = Budget::builder().build().unwrap().id();

    assert_ne!(first, second);
    assert_eq!(first.to_string(), first.get().to_string());
    assert!(second.get() > first.get());
}

#[test]
fn exhaustion_error_contains_the_limiting_scope_metadata() {
    let units = Resource::new("units").unwrap();
    let root = Budget::builder()
        .name("account")
        .limit(units.clone(), 1)
        .build()
        .unwrap();
    let child = root.child().name("task").build().unwrap();

    let error = child.consume(&units, 2).unwrap_err();
    assert_eq!(
        error,
        BudgetError::Exhausted {
            resource: units,
            requested: 2,
            remaining: 1,
            scope: root.id(),
            scope_name: Some("account".into()),
        }
    );
    assert!(error.to_string().contains("requested 2, remaining 1"));
}
