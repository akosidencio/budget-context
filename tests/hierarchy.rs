//! Parent, child, grandchild, and sibling accounting tests.

use std::time::{Duration, Instant};

use budget_context::{Budget, BudgetError, Remaining, Resource};

#[test]
fn descendants_update_every_ancestor_and_compete_as_siblings() {
    let tokens = Resource::new("tokens").unwrap();
    let root = Budget::builder()
        .name("root")
        .limit(tokens.clone(), 100)
        .build()
        .unwrap();
    let left = root
        .child()
        .name("left")
        .limit(tokens.clone(), 80)
        .build()
        .unwrap();
    let right = root
        .child()
        .name("right")
        .limit(tokens.clone(), 80)
        .build()
        .unwrap();
    let grandchild = left.child().name("grandchild").build().unwrap();

    grandchild.consume(&tokens, 70).unwrap();
    right.consume(&tokens, 30).unwrap();

    assert_eq!(root.remaining(&tokens), Remaining::Limited(0));
    assert_eq!(left.remaining(&tokens), Remaining::Limited(0));
    assert_eq!(right.remaining(&tokens), Remaining::Limited(0));
    assert!(matches!(
        right.consume(&tokens, 1),
        Err(BudgetError::Exhausted { scope, .. }) if scope == root.id()
    ));

    assert_eq!(root.snapshot().resources[0].consumed, 100);
    assert_eq!(left.snapshot().resources[0].consumed, 70);
    assert_eq!(grandchild.snapshot().resources[0].consumed, 70);
}

#[test]
fn looser_child_limit_never_expands_parent() {
    let tokens = Resource::new("tokens").unwrap();
    let root = Budget::builder().limit(tokens.clone(), 5).build().unwrap();
    let child = root.child().limit(tokens.clone(), 100).build().unwrap();

    assert_eq!(child.remaining(&tokens), Remaining::Limited(5));
    child.consume(&tokens, 5).unwrap();
    assert!(child.consume(&tokens, 1).is_err());
}

#[test]
fn snapshots_include_lineage_limits_local_limits_and_observed_resources() {
    let inherited = Resource::new("inherited").unwrap();
    let local = Resource::new("local").unwrap();
    let observed = Resource::new("observed").unwrap();
    let root = Budget::builder()
        .limit(inherited.clone(), 100)
        .build()
        .unwrap();
    let child = root
        .child()
        .name("child")
        .limit(local.clone(), 20)
        .build()
        .unwrap();
    child.consume(&observed, 3).unwrap();

    let snapshot = child.snapshot();
    let names: Vec<_> = snapshot
        .resources
        .iter()
        .map(|entry| entry.resource.as_str())
        .collect();
    assert_eq!(names, ["inherited", "local", "observed"]);

    let inherited_entry = &snapshot.resources[0];
    assert_eq!(inherited_entry.local_limit, None);
    assert_eq!(inherited_entry.effective_remaining, Remaining::Limited(100));
    let local_entry = &snapshot.resources[1];
    assert_eq!(local_entry.local_limit, Some(20));
    assert_eq!(local_entry.effective_remaining, Remaining::Limited(20));
    let observed_entry = &snapshot.resources[2];
    assert_eq!(observed_entry.consumed, 3);
    assert_eq!(observed_entry.effective_remaining, Remaining::Unlimited);
}

#[test]
fn children_inherit_or_shorten_but_never_extend_deadlines() {
    let parent_deadline = Instant::now() + Duration::from_secs(60);
    let root = Budget::builder()
        .deadline_at(parent_deadline)
        .build()
        .unwrap();
    let inherited = root.child().build().unwrap();
    let attempted_extension = root
        .child()
        .deadline_after(Duration::from_secs(120))
        .build()
        .unwrap();
    let shortened = root
        .child()
        .deadline_after(Duration::from_secs(1))
        .build()
        .unwrap();

    for budget in [&inherited, &attempted_extension] {
        let remaining = budget.snapshot().deadline_remaining.unwrap();
        assert!(remaining > Duration::from_secs(50));
        assert!(remaining <= Duration::from_secs(60));
    }
    assert!(shortened.snapshot().deadline_remaining.unwrap() <= Duration::from_secs(1));
}
