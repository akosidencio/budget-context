# Design contract

This document records the semantics that are expensive for downstream users to
infer from implementation details.

## Core promise

Given cooperative callers in one process, no successful accounting operation
causes a limited resource to violate:

```text
consumed + reserved <= limit
```

Every operation on a node updates that node and all ancestors atomically. The
implementation collects the lineage, locks it root-to-leaf, validates every
resource, and mutates only after all validation succeeds.

## Hierarchy

- A node's counters include its complete subtree.
- A child limit is a local ceiling, not a capacity allocation.
- Siblings compete for shared ancestor capacity.
- A looser child limit never expands an ancestor limit.
- If no node in a lineage limits a resource, it is unlimited but still tracked.
- No fairness or starvation guarantee is provided between siblings.

## Operation order and errors

Zero-quantity consume and reserve operations are successful no-ops. Other
operations use this precedence:

1. canonicalization and checked-arithmetic errors;
2. cancellation, when the `tokio` feature is active;
3. an elapsed effective deadline;
4. capacity checks in root-to-leaf node order and resource-name order.

Duplicate multi-resource entries are combined. When several malformed entries
exist, the lexicographically first resource is reported, independently of input
iterator order. Poisoned standard mutexes are recovered; accounting code does
not expose user callbacks while holding a lineage lock.

Deadlines and cancellation are checked when an operation begins. A terminal
condition may race with an operation already admitted. Existing reservations
can always reconcile or release after a deadline or cancellation request.

## Reservations

Dropping an active reservation releases all of it. Committing converts actual
usage to consumed capacity and releases unused capacity.

External usage above a reservation cannot be admitted retroactively without
breaking the core invariant. An overage therefore converts the full reserved
amount to consumed capacity and returns `ReservationExceeded` with the
unaccounted difference. Callers requiring a hard bound must constrain the
underlying operation to the reserved maximum.

For a reservation set:

- omitted actual resources mean zero usage;
- unknown resources and actual-entry overflow fail closed by consuming the
  complete reservation set;
- valid resources reconcile atomically;
- if several resources exceed their reservations, reconciliation occurs for
  all resources and the lexicographically first overage is returned.

## Observation

`remaining()` and snapshots are informational, never authorization. A snapshot
locks the lineage root-to-leaf and describes one consistent instant. It lists
the union of resources limited anywhere in the lineage and resources observed
in the selected node's subtree.

## Tokio execution

`Budget::run()` returns `Result<F::Output, BudgetError>`. A fallible wrapped
future therefore produces a nested result. When branches are ready together,
cancellation wins over deadline expiration, which wins over future completion.
The wrapped future is dropped on cancellation or timeout and must itself be
cancellation-safe.

`CancellationToken` provides directional propagation: cancelling a parent
cancels descendants; cancelling a child does not affect ancestors or siblings.

## Complexity and non-goals

Accounting is `O(lineage depth × resource count)` and root contention
serializes sibling operations. The crate intentionally does not provide:

- distributed or durable quotas;
- a billing ledger;
- rate limiting or capacity replenishment;
- fair concurrency permits or waiting queues;
- permission checks or sandboxing;
- enforcement against code which ignores the budget.

