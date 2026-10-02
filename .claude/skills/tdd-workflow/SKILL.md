---
name: tdd-workflow
description: Red, green, refactor for any new feature, bug fix or lint rule in notaio. Load before writing implementation code: the failing test comes first.
---

# TDD Workflow

For an architectural change the ADR comes first
(`rules/architecture-driven-development.md`); then this.

## RED: a failing test first

```sh
# Unit: crates/<crate>/src/<module>_tests.rs
# Crate contract: crates/<crate>/tests/<name>.rs
cargo test -p <crate> <test_name>     # must FAIL, for the reason you expect
```

A test that passes before the implementation exists is testing nothing. Read
the failure message: it should name the missing behaviour.

## GREEN: the minimum code that passes

```sh
cargo test -p <crate> <test_name>     # must PASS now
```

## REFACTOR: improve while green

Extract constants, add rustdoc, tighten errors. Then the full gate:

```sh
make fmt clippy test manifests-check  # the cargo-quality skill
```

## Test file pattern

- `src/foo.rs` ends with `#[cfg(test)] mod foo_tests;`
- `src/foo_tests.rs` starts with `use super::*;`

## notaio specifics

- **`sandboxpolicy-core` is pure**, so pass `now` in and pin it in the test.
  Never read the clock in a test of core logic.
- **A new lint rule** gets a stable code (`L0NN`), rustdoc stating its meaning,
  and a test that fails when the rule is weakened. Write that test first: a
  policy the rule must reject, asserted by code.
- **A bug that only shows on a cluster** gets a failing e2e first
  (`crates/notaio/tests/e2e_<suite>.rs`, `make kind-e2e-<suite>`), then, if
  the decision is offline-decidable, a unit test that pins it cheaply.

## Verification

All tests pass, clippy is clean, and the tests cover the success path, each
failure path and the edge cases.
