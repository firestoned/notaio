# Testing Standards

## CRITICAL: Test-Driven Development (TDD)

**Write the test first, every time.** Red, green, refactor, per the
`tdd-workflow` skill. TDD sits inside ADD (`rules/architecture-driven-development.md`):
for an architectural change the ADR comes first, then the tests.

- **New feature**: a test that defines the behaviour, then the code
- **Bug fix**: a test that reproduces the bug and fails, then the fix
- **Refactor**: existing tests green before and after; add edge cases you find
- **New lint rule**: lands with a test that fails when the rule is weakened
  (CLAUDE.md). A rule without one can be loosened silently.

Exceptions: exploratory code (marked as such, removed before merge) and
behaviour-preserving refactors the existing tests already cover.

---

## After Modifying Any `.rs` File

Run the `cargo-quality` skill. The task is not complete until it passes. Then
check:

1. **Rustdoc matches the code**: `# Arguments`, `# Errors`, examples.
2. **Tests match the change**: added for new behaviour, updated for changed
   behaviour, deleted with deleted code.
3. **Generated files are current**: a change to a CRD type, the knob register
   or a built-in profile needs `make manifests` (the `regen-manifests` skill);
   a change to `docs/air/` needs `make air`.
4. **Docs and CHANGELOG** reflect it (`rules/documentation.md`).

---

## Unit Testing Requirements

Every public function has tests: success path, each failure path, edge cases.
Descriptive names (`grant_two_steps_above_profile_is_rejected`),
Arrange-Act-Assert, deterministic. `sandboxpolicy-core` is pure, which makes
this cheap: time comes in as `now`, so a test sets the clock, never reads it.

### Test file organisation

New unit tests go in a separate `_tests.rs` file, not inline in the source:

- `src/foo.rs` ends with `#[cfg(test)] mod foo_tests;`
- `src/foo_tests.rs` holds `use super::*;` and the tests

`crates/sandboxpolicy-core/src/lint.rs` still carries an inline
`mod tests`; move it to `lint_tests.rs` the next time that module is touched
for another reason, not as a drive-by.

Tests may use `unwrap` and `expect`; open the file with
`#![allow(clippy::unwrap_used, clippy::expect_used)]` as the existing
`tests/` files do. Production code may not (CLAUDE.md).

---

## The three tiers, and which one to reach for

Each tier proves things the one below cannot. Picking the wrong one gives a
test that passes for the wrong reason.

| Tier | Where | Needs | Proves | Run with |
| --- | --- | --- | --- | --- |
| **Unit** | `src/*_tests.rs` | nothing | every decision, as a pure function | `make test` |
| **Integration** | `crates/*/tests/*.rs` (`evaluate.rs`, `bundle.rs`) | nothing | a crate's public contract end to end: evaluate a policy, sign and verify a bundle | `make test` |
| **E2E** | `crates/notaio/tests/e2e_*.rs` | a kind cluster, notaio deployed from `deploy/` | that the API server accepts what notaio writes, that the deployed RBAC and admission policy are what the threat model says, that a consumer can verify the published bundle | `make kind-e2e-<suite>`, `make kind-e2e` |

E2E suites are `#[ignore]`d so `make test` stays offline. Each has its own
Makefile target and its own CI job (`rules/github-workflows.md`).

### Rules these tiers exist to enforce

1. **A fake that is more permissive than the real thing hides bugs.** When a
   test double stands in for the API server, a signer or a consumer, check it
   rejects what the real one rejects. When an e2e finds a behaviour a fake got
   wrong, fix the fake in the same change.

2. **A test that skips must never report success.** Running an ignored e2e is
   an explicit request to talk to a cluster, so an unreachable cluster or a
   missing fixture is a **failure** that names what is missing. A test that
   returns early and prints `ok` answers "is this covered?" with a confident,
   wrong yes.

3. **Wait on what is actually new.** A wait that stale state can satisfy is
   not a wait. The bundle e2e waits for a version *strictly above* the last
   one it saw, not for `Ready`, which was already true before the change.

4. **No fixed dates in fixtures.** A grant that expires on a literal date turns
   the suite red on that day. Compute expiries from `now`.

5. **Anything decidable offline belongs in a unit test.** The e2e is for the
   questions an offline test cannot answer.

---

## Test Execution

```sh
make test                          # unit + integration, offline
cargo test -p <crate> <name>       # one test, inner loop
make kind-e2e-bundle               # one e2e suite on a local kind cluster
make kind-e2e                      # every e2e suite
```

All tests must pass before the work is complete.
