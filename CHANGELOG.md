# Changelog

All notable changes to `budget-context` are documented in this file.

The project follows [Semantic Versioning](https://semver.org/). Until `1.0`,
minor versions may introduce API changes; patch versions remain compatible with
their corresponding minor release.

## [Unreleased]

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

[Unreleased]: https://github.com/thinkgrid-labs/budget-context/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/thinkgrid-labs/budget-context/releases/tag/v0.1.0
