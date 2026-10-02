---
name: cargo-quality
description: Mandatory quality gate after ANY Rust change: fmt, clippy -D warnings, the workspace tests, and the generated-manifest check. The task is NOT complete until all pass. Load at the end of every task that touched a .rs file, before updating the changelog or declaring the work done.
---

# Cargo Quality Gate

Run these from the workspace root, in this order. Every one must pass: a task
that modified any `.rs` file is **not complete** until they do.

```sh
make fmt               # cargo fmt --all
make clippy            # cargo clippy --workspace --all-targets -- -D warnings
make test              # cargo test --workspace (e2e suites are #[ignore]d)
make manifests-check   # deploy/crds and deploy/profiles match the Rust types
```

CI runs the same targets (`make fmt-check` in place of `make fmt`), so green
here is green there.

## Rules

1. **fmt first.** It can reflow code that clippy would then report differently.
2. **Clippy warnings are errors.** Fix every one. Never `#[allow(...)]` one
   away without a comment stating the constraint that justifies it, and never
   weaken `-D warnings`. `unwrap_used` and `expect_used` are workspace lints:
   production code handles the error, tests opt out at the top of the file.
3. **All tests, not just the crate you touched.** A type change in
   `sandboxpolicy-types` ripples into every crate. Scope with `-p <crate>` for
   the inner loop only.
4. **Fix, re-run, repeat.** The gate is green output, not a first attempt.

## After the gate passes, verify

- Rustdoc on every public item you touched still matches what it does.
- Tests were added, updated or deleted to match (`rules/testing.md`).
- No magic numbers introduced (`rules/rust-style.md`).
- A CRD type, knob or profile changed: `make manifests` was run and the
  generated files are in the change (the `regen-manifests` skill).
- `sandboxpolicy-types` and `sandboxpolicy-bundle` gained no dependency on
  `kube` or `k8s-openapi`:
  `cargo tree -p sandboxpolicy-bundle -e normal | rg 'kube|k8s-openapi'` prints nothing.
- A new dependency passes `make deny`.

## Gotchas

- `cargo clippy` without `-- -D warnings` exits 0 on warnings. Use `make clippy`.
- The e2e suites (`crates/notaio/tests/e2e_*.rs`) compile under `make test` but
  only run with `--ignored` against a cluster: `make kind-e2e-<suite>`. Do not
  try to make them pass here.
- On the Mac, a cold build competes for a nearly full disk. Prefer building on
  grill (`~/dev/CLAUDE.md`).
