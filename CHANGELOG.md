# Changelog

All notable changes to `budget-context` are documented in this file.

The project follows [Semantic Versioning](https://semver.org/). Until `1.0`,
minor versions may introduce API changes; patch versions remain compatible with
their corresponding minor release.

## [0.2.0] - October 05, 2026

### Breaking

- `BudgetError`, `BudgetBuildError`, `ResourceError`, `BudgetSnapshot`, and
  `ResourceSnapshot` are `#[non_exhaustive]`. Matches need a wildcard arm and
  snapshots can no longer be built with struct literals.
- `BudgetError::Cancelled` and `BudgetSnapshot::cancelled` exist without the
  `tokio` feature, so enabling a feature anywhere in a build no longer changes
  these types. Without `tokio` the error is never returned and the field is
  always `false`.
- `ReservationSet::amounts` lists resources requested with a zero quantity.
  Usage reported for them is a `ReservationExceeded` overage, matching a
  zero-quantity `Reservation`, instead of `UnknownReservationResource`.

### Changed

- Drop the unused `rt` feature of `tokio-util`, which pulled in `tokio/rt`
  and `futures-util` for the `tokio` feature.

### Fixed

- Dropping a deep budget lineage no longer overflows the stack and aborts the
  process; ancestors are now released iteratively.
- `Budget`'s `Debug` output reports the parent by identifier instead of
  recursing through every ancestor.
- Deserializing a `Resource` now rejects empty names, as `Resource::new` does.
- `ReservationSet::commit` ignores zero-quantity actual entries. Previously a
  zero entry for a resource requested with a zero quantity failed closed and
  consumed the whole reservation set.

### Internal

- The Loom model now exercises the crate's own lineage locking instead of a
  stand-alone model of it.
- CI checks dependency licenses and advisories with `cargo deny`, runs the
  Loom model, and pins every action to a commit.
- Document that budgets retain an entry per distinct resource name.
- Add a tested example and feature overview to the docs.rs landing page, and
  label feature-gated items on docs.rs.
- Exclude the README banner and CI configuration from the published package;
  crates.io loads README images from the repository.
- Build warning-free on Rust 1.99, which deprecates `AtomicU64::fetch_update`.
- The SemVer job compares against the latest crates.io release and infers the
  release type from the crate version instead of hard-coding both.

## [0.1.1] - August 17, 2026

### Changed

- Build docs.rs documentation with all optional features enabled.
- Correct the Tokio installation example for crates.io users.
- Compile-check every feature combination in CI.
- Check patch releases for public API compatibility with `0.1.0`.
- Compile-test Rust examples embedded in the README.
- Add linked crates.io, docs.rs, CI, and license badges.

## [0.1.0]

- Initial release of hierarchical, process-local resource budgets.
- Atomic accounting across complete budget lineages.
- Single-resource and multi-resource RAII reservations.
- Optional Tokio cancellation, deadlines, Serde, and tracing integrations.

[0.2.0]: https://github.com/akosidencio/budget-context/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/akosidencio/budget-context/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/akosidencio/budget-context/releases/tag/v0.1.0
