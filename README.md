# budget-context

<p align="center">
  <img src="./assets/banner.jpg" alt="budget-context — hierarchical resource budgets for concurrent and async Rust task trees" width="100%">
</p>

<p align="center">
  <strong>Atomic, hierarchical resource budgets for concurrent Rust task trees.</strong><br>
  Bound tokens, tool calls, API requests, bytes, retries, cost estimates, deadlines, and any other integer resource.
</p>

<p align="center">
  <a href="https://crates.io/crates/budget-context"><img alt="crates.io" src="https://img.shields.io/crates/v/budget-context.svg?logo=rust"></a>
  <a href="https://docs.rs/budget-context"><img alt="docs.rs" src="https://docs.rs/budget-context/badge.svg"></a>
  <a href="https://github.com/akosidencio/budget-context/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/akosidencio/budget-context/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="Rust 1.85 or newer" src="https://img.shields.io/badge/Rust-1.85%2B-CE412B?logo=rust&logoColor=white">
  <a href="./LICENSE"><img alt="MIT licensed" src="https://img.shields.io/crates/l/budget-context.svg"></a>
  <img alt="Unsafe Rust forbidden" src="https://img.shields.io/badge/unsafe-forbidden-6F42C1">
  <img alt="Project status: early release" src="https://img.shields.io/badge/status-early%20release-E3A008">
</p>

`budget-context` is a dependency-light Rust crate for enforcing cumulative,
process-local resource limits across concurrent and asynchronous work. A child
task may impose a tighter budget, but it cannot escape its parent's limits.
Accounting and reservations remain atomic across the complete task hierarchy.

It is designed for AI agents, autonomous workflows, web crawlers, batch jobs,
request-scoped quotas, and any Rust system where the total amount of work is
unknown before execution begins.

## Contents

- [Why budget-context?](#why-budget-context)
- [Core capabilities](#core-capabilities)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Multi-resource reservations](#multi-resource-reservations)
- [Tokio cancellation and deadlines](#tokio-cancellation-and-deadlines)
- [Where it fits](#where-it-fits)
- [How it compares](#how-it-compares)
- [Safety and non-goals](#safety-and-non-goals)
- [Feature flags](#feature-flags)
- [Performance](#performance)
- [Development](#development)

## Why budget-context?

Long-running and autonomous programs rarely follow a predictable path:

```text
root workflow: 100k tokens, 50 tool calls, 5 minute deadline
│
├── research agent: 25k tokens, 15 searches
│   └── web worker
│
├── coding agent: 60k tokens, 20 test runs
│   └── test worker
│
└── reviewer: 20k tokens
```

Checking `remaining()` before doing work is racy: another task may consume the
same capacity between the check and the update. Independent counters also fail
when sibling tasks collectively exceed a shared parent limit.

`budget-context` solves those two problems with one primitive:

> Every successful resource operation is validated and recorded atomically at
> the current node and every ancestor.

For each limited resource, the crate preserves:

```text
consumed + reserved <= limit
```

## Core capabilities

- **Arbitrary resources** — applications define names such as `llm.tokens`,
  `agent.tool_calls`, `http.requests`, or `cost.usd_micros`.
- **Hierarchical limits** — children inherit ancestor constraints and may add
  tighter local ceilings.
- **Atomic accounting** — concurrent consume and reserve operations cannot
  oversubscribe a resource.
- **RAII reservations** — unused capacity is automatically released when a
  reservation is dropped.
- **Multi-resource transactions** — reserve tokens, requests, and estimated
  cost together, or reserve none of them.
- **Fail-closed reconciliation** — reported usage above a reservation retains
  the full reservation as consumed and reports the unaccounted overage.
- **Consistent snapshots** — inspect consumed, reserved, locally limited, and
  effectively remaining capacity at one instant.
- **Deadlines** — child deadlines may shorten, but never extend, an ancestor's
  deadline.
- **Optional Tokio integration** — directional cancellation propagation and
  deadline-aware future execution.
- **Optional Serde and tracing** — serialize observational data and emit
  structured accounting events without coupling to a backend.

## Installation

Add the core crate from crates.io:

```sh
cargo add budget-context
```

Enable integrations only when needed:

```sh
cargo add budget-context --features tokio,serde,tracing
```

For a local checkout or workspace under development:

```toml
[dependencies]
budget-context = { path = "../budget-context" }
```

The accounting core does not require an async runtime.

## Quick start

```rust
use budget_context::{Budget, Resource};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tokens = Resource::new("llm.tokens")?;
    let tool_calls = Resource::new("agent.tool_calls")?;

    let task = Budget::builder()
        .name("coding-task")
        .limit(tokens.clone(), 100_000)
        .limit(tool_calls.clone(), 50)
        .build()?;

    let researcher = task
        .child()
        .name("researcher")
        .limit(tokens.clone(), 25_000)
        .build()?;

    researcher.consume(&tool_calls, 1)?;

    let reservation = researcher.reserve(&tokens, 8_000)?;
    // Constrain the external operation to the reserved maximum.
    let actual_tokens = 3_421;
    reservation.commit(actual_tokens)?;

    println!("{:#?}", researcher.snapshot());
    Ok(())
}
```

`remaining()` and snapshots are observational. Only `consume()`, `reserve()`,
and `reserve_many()` authorize work atomically.

## Multi-resource reservations

Operations often consume several resources together. An LLM request may need
one request slot, a token allowance, and an estimated-cost allowance:

```rust
# use budget_context::{Budget, Resource};
# fn example() -> Result<(), Box<dyn std::error::Error>> {
# let tokens = Resource::new("llm.tokens")?;
# let requests = Resource::new("llm.requests")?;
# let cost = Resource::new("cost.usd_micros")?;
# let budget = Budget::builder()
#     .limit(tokens.clone(), 100_000)
#     .limit(requests.clone(), 10)
#     .limit(cost.clone(), 2_000_000)
#     .build()?;
let permit = budget.reserve_many([
    (&requests, 1),
    (&tokens, 8_000),
    (&cost, 100_000),
])?;

permit.commit([
    (&requests, 1),
    (&tokens, 3_421),
    (&cost, 42_000),
])?;
# Ok(())
# }
```

Acquisition and reconciliation are atomic across every requested resource and
every node in the lineage. Duplicate entries are combined using checked
arithmetic. A zero-quantity entry reserves nothing but stays in the set, so
usage reported for it is an overage. Dropping `permit` without committing
releases all reserved capacity.

### Why overages fail closed

If an external operation reports 9,000 tokens after reserving 8,000, the crate
cannot admit the extra 1,000 retroactively without breaking an ancestor limit.
It therefore converts the full 8,000 reservation to consumed capacity and
returns `ReservationExceeded` with the 1,000-token unaccounted overage.

Applications requiring a hard bound must also configure the underlying
operation—such as a model's maximum output tokens—to stay within the reserved
amount.

## Tokio cancellation and deadlines

Enable the `tokio` feature:

```toml
[dependencies]
budget-context = { version = "0.2", features = ["tokio"] }
```

Then run a future until it completes, the budget is cancelled, or its effective
deadline expires:

```rust,no_run
# use std::time::Duration;
# use budget_context::Budget;
# async fn operation() -> Result<u64, std::io::Error> { Ok(42) }
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let budget = Budget::builder()
    .deadline_after(Duration::from_secs(30))
    .build()?;

let value = budget.run(operation()).await??;
# let _ = value;
# Ok(())
# }
```

`run()` preserves the future's output, so a fallible future produces a nested
`Result`. Cancellation wins over deadline expiration, which wins over future
completion when multiple branches are ready together.

Cancelling a parent cancels its descendants. Cancelling a child does not cancel
its parent or siblings. Existing reservations may still reconcile or release
after cancellation so accounting is never abandoned halfway through.

## Where it fits

Good uses include:

- **AI agents and autonomous workflows** — tokens, model requests, tool calls,
  searches, shell commands, test runs, and estimated cost.
- **MCP and plugin hosts** — bound operations at host-controlled invocation
  boundaries.
- **Web crawlers** — HTTP requests, downloaded bytes, pages, and recursion.
- **Server requests** — database queries, downstream API calls, response bytes,
  and request deadlines.
- **Batch processing** — records, retries, errors, external calls, and
  speculative work.
- **Recursive algorithms** — depth, nodes, expansions, or generated items.

## How it compares

| Tool | Best for | Hierarchy | RAII reservations | Replenishes over time | Waits fairly |
| --- | --- | :---: | :---: | :---: | :---: |
| **budget-context** | Cumulative task-tree budgets | yes | yes | no | no |
| [`qubit-budget`](https://crates.io/crates/qubit-budget) | Lightweight single-dimension accounting | no | no | no | no |
| [`governor`](https://crates.io/crates/governor) | Rate limiting and replenishing quotas | no | no | yes | optional async wait |
| [`tokio::sync::Semaphore`](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html) | Concurrent in-flight work | no | permit on drop | reusable permits | yes |
| [`CancellationToken`](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html) | Hierarchical cancellation | cancellation only | no | n/a | n/a |

These tools are complementary. Use a rate limiter for requests per second, a
semaphore for maximum in-flight operations, and `budget-context` for the total
amount of work allowed in one execution tree.

## Safety and non-goals

`budget-context` is cooperative. Code must receive and consult a `Budget` to be
constrained. It does not sandbox untrusted code or replace authorization.

The crate is intentionally not:

- a distributed or persistent quota service
- a monthly user or organization quota system
- a billing or audit ledger
- an API rate limiter
- a fair concurrency limiter or waiting queue
- a permission system or security boundary
- a mechanism for measuring CPU time or memory exactly
- a way to retroactively prevent external usage beyond an unenforced estimate

Live `Budget` values are not serializable. They contain process-local clocks,
locks, hierarchy state, and optional cancellation tokens. Read-only snapshots
can be serialized with the `serde` feature.

## Feature flags

| Feature | Default | Adds |
| --- | :---: | --- |
| `serde` | no | Serialization for resources, snapshots, and errors |
| `tokio` | no | Hierarchical cancellation and `Budget::run()` |
| `tracing` | no | Structured consume, reserve, commit, release, and cancel events |

The exact hierarchy, error ordering, reconciliation, snapshot, and runtime
semantics are recorded in [DESIGN.md](./DESIGN.md).

## Performance

Accounting is `O(hierarchy depth × resource count)` and locks each lineage in a
stable root-to-leaf order. The initial implementation prioritizes correctness,
deadlock prevention, and deterministic errors over lock-free complexity.

The included Criterion benchmarks cover:

- hierarchy depths 1, 2, 5, and 10
- single and multi-resource reservations
- contention with 1, 4, 16, and 64 workers

Use operation-level accounting rather than calling the crate for every byte or
item in a very hot loop. No fairness guarantee is made between sibling tasks.

Budgets keep an entry for every distinct resource name they have been charged,
so define resources from a fixed vocabulary rather than from unbounded input
such as request identifiers.

## Development

The repository forbids unsafe Rust and checks all public documentation.

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo hack check --feature-powerset --no-dev-deps
cargo bench --all-features
cargo doc --all-features --no-deps
cargo package
cargo deny --all-features check
cargo llvm-cov --all-features --workspace --fail-under-lines 95
RUSTFLAGS="--cfg budget_context_loom" cargo test --release --test loom_model
```

The test suite includes accounting, hierarchy, reservation, concurrency, Tokio,
Serde, tracing, public API, and property coverage, plus a Loom model that
explores every interleaving of the crate's own lineage locking. CI tests stable
Rust on Linux, macOS, and Windows, checks the Rust 1.85 MSRV, runs both feature
matrices and every feature combination, guards the public API against SemVer
regressions, checks dependency licenses and advisories with `cargo deny`,
compiles benchmarks and README examples, and enforces a 95% line-coverage
floor.

Contributions are welcome while the API is being shaped. Please include tests
for semantic changes and preserve the invariants in [DESIGN.md](./DESIGN.md).

## License

Licensed under the [MIT License](./LICENSE).
