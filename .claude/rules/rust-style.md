# Rust Style Guide

## Core Principles

- `thiserror` for error types in libraries; `anyhow` only in the `notaio` binary
- `tracing` for logging, never `println!` or `log`
- `tokio` for async
- **No `unwrap` or `expect` outside tests**, no `unsafe` (`unsafe_code = "forbid"`
  in the workspace lints). CLAUDE.md.
- **`sandboxpolicy-core` is pure**: no I/O, no clock reads, no network. Time
  comes in as `now`.
- **`sandboxpolicy-types` and `sandboxpolicy-bundle` never depend on `kube` or
  `k8s-openapi`.** mediatore-guest runs as root in every sandbox VM and links
  the bundle crate; every dependency there is attack surface in a root process.
- **Early returns and guard clauses**, not nesting
- **No magic numbers**: any numeric literal other than `0` or `1` is a named
  constant

---

## Early Return / Guard Clause Pattern

Handle the edge case first and leave; keep the main path at the lowest
indentation.

```rust
// ✅ GOOD
pub fn from_env() -> anyhow::Result<Option<SharedSigner>> {
    let Ok(path) = std::env::var("NOTAIO_SIGNING_KEY_FILE") else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
    Ok(Some(Arc::new(Ed25519Signer::from_seed_b64(&key_id, &raw)?)))
}

// ❌ BAD
pub fn from_env() -> anyhow::Result<Option<SharedSigner>> {
    if let Ok(path) = std::env::var("NOTAIO_SIGNING_KEY_FILE") {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            // ... the real work, three levels deep
        } else { /* ... */ }
    } else {
        Ok(None)
    }
}
```

- Validate inputs at the top and return
- `let ... else` and `?` over nested `match` / `if let`
- `continue` early in loops (`reconcile_policy` skips a grant it cannot find)
- Fail closed: the early return on a security check is the refusal. Bundle
  verification returns an error on the first check that fails; it never
  collects "warnings" and carries on.

---

## Magic Numbers Rule

Any numeric literal other than `0` or `1` is a named constant, with a comment
saying where the value comes from. In this repository most numbers come from
the threat model, and the constant is where a reader finds that out.

```rust
// ✅ GOOD
/// Default bundle lifetime. A consumer refuses a bundle past `notAfter`, so
/// this bounds how long a revoked relaxation can survive (ADR-0002).
const DEFAULT_BUNDLE_LIFETIME_SECONDS: i64 = 3600;

// ❌ BAD
.unwrap_or(3600);
```

- Knob steps, TTL ceilings and lease maxima belong in `registry.rs` and
  `ceilings.rs`, cited to the model, never inline.
- Test files are exempt: literal fixtures are clearer there.
- Existing code predates this rule (`crates/notaio/src/main.rs` still has a
  bare `3600`); fix a site when you are already changing it.

Find candidates:

```sh
rg -n '\b[2-9][0-9]*\b|\b1[0-9]+\b' crates/*/src --glob '!*_tests.rs'
```

---

## Repeated Strings Become Constants

A string used in more than one place, or one that is part of a contract, is a
`const`: condition types, annotation keys, the bundle key, the field manager,
env var names. `reconcile.rs` already does this for `ANN_VERSION`,
`ANN_DIGEST`, `BUNDLE_KEY` and `FIELD_MANAGER`. A consumer depends on those
exact strings; a typo in one place is a silent contract break.

---

## Dependency Management

Before adding a dependency:

1. Check whether an existing one already solves it.
2. Screen the licence: permissive and foundation friendly only. No BSL, no
   AGPL, nothing under single-vendor control. Run it past `deny.toml`
   (`make deny`). CLAUDE.md.
3. Check it is maintained (commits in the last six months).
4. **Never add one to `sandboxpolicy-types` or `sandboxpolicy-bundle`
   casually**: see Core Principles.
5. Record why in `.claude/CHANGELOG.md`.

---

## Code Comments

Public functions and types have rustdoc. Say what it does, what it refuses and
why, citing the threat model or ADR where the reason lives:

```rust
/// Verify an envelope and return the payload. Fails closed on every check.
///
/// # Errors
/// Returns `BundleError` for a wrong payload type, a signature from no trusted
/// key, an expired bundle, or a version below `opts.min_version` (T5.4).
pub fn verify_bundle(
```

No em-dashes in comments or docs (CLAUDE.md, working rule 6).

---

## Things to Never Do

- **Never** publish an unsigned bundle, or add a code path that could
- **Never** give the controller create, update or delete on policy objects;
  its RBAC is read plus status writes (the `e2e_rbac` suite enforces this)
- **Never** add hypervisor credentials, agent traffic or an inbound listener
  to the controller
- **Never** loosen a lint rule, ceiling or knob maximum without a threat model
  change to point to
- **Never** hardcode a namespace; take it from the object or configuration
- **Never** `sleep` to synchronise in the controller; watch
- **Never** hand edit `deploy/crds/` or `deploy/profiles/`
