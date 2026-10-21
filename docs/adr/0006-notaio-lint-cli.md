# ADR-0006: `notaio lint`, the policy check for pull requests

- **Status:** Accepted
- **Date:** 2026-10-20
- **Deciders:** Erick Bourgeois
- **Related:** ROADMAP M1 ("`notaio lint` CLI ... for pull request checks"); mediatore ADR-0014
  and banlieue ADR-0004 (one binary, roles as subcommands)

Implemented with this ADR: `crates/notaio/src/lint.rs`, `crates/notaio/src/cli.rs`, tests in
`crates/notaio/tests/lint.rs` and `cli.rs`.

## Context

The threat model asks for a "toxic-combination lint on every policy pull request". Today the lint
rules only run in the controller, after a change has merged and the GitOps reconciler has applied
it: a bad policy is reported in its status, not stopped in review. The rules are in
`sandboxpolicy-core`, which is pure and has no I/O, so running the same checks on files needs only
a thin loader around them.

Policy repositories hold SandboxPolicy and KnobGrant objects and may hold their own
SandboxProfiles, next to other manifests (kustomizations, namespaces). Reviewers on GitHub see
code scanning annotations inline; CodeQL and Semgrep already upload SARIF for this repository.

## Decision

1. **One binary, two subcommands.** `notaio controller` runs the controller, exactly as the bare
   `notaio` does today; `notaio lint` checks files. Both ship in the one image, so a policy
   repository runs the published image (`notaio lint <dir>`) without building anything. The
   deployment passes `controller` explicitly, and a test parses the arguments the manifests use.
   A bare `notaio` prints usage.
2. **The same code decides.** `notaio lint` runs `sandboxpolicy_core::evaluate` on every policy,
   with its grants and its profile, under the default ceilings, and `lint_knobs` on every
   profile. Nothing is reimplemented, so the file check and the controller cannot disagree.
3. **Inputs** are files and directories (recursively, `*.yaml` and `*.yml`), multi-document.
   Documents outside the `sandbox.firestoned.io` group are counted and ignored. A grant belongs to
   the policy its `policyRef` names in the same namespace.
4. **Profiles come from the files first, then the built-ins.** A profile defined in the inputs
   wins; one they do not define falls back to the four built-in profiles compiled into notaio.
5. **The clock is now,** so an expired grant fails a pull request as it would lapse in the
   cluster. `--now <RFC 3339>` pins it for reproducible runs and tests.
6. **Output is text by default, SARIF 2.1.0 with `--format sarif`.** Every finding carries its
   stable code as the SARIF rule id, the file as the location, and the object and field in the
   message. Exit code 0 when there is no error (warnings allowed), 1 when any finding is an error,
   2 when an input cannot be read or parsed as YAML.
7. **Input problems have stable codes too**, alongside the lint rules L001 to L013:

   | Code | Severity | Meaning |
   | ---- | -------- | ------- |
   | I001 | error | An object in the `sandbox.firestoned.io` group is not a valid SandboxProfile, SandboxPolicy or KnobGrant |
   | I002 | error | The same object (kind, namespace, name) appears more than once in the inputs |
   | I003 | error | A policy references a profile that is neither in the inputs nor built in |
   | I004 | error | A grant references a policy that is not in the inputs |
   | I005 | error | The compiled content has no RFC 8785 canonical form (ADR-0005) |
   | I006 | error | A grant has expired |
   | I007 | error | A policy's profile does not lint clean; the profile's own findings are reported against it |

   Each has a test that fails when the check is removed.

## Consequences

- A policy that would be rejected in the cluster now fails its pull request, with the same codes,
  and on GitHub the reviewer sees the finding on the file.
- `GrantResult` in `sandboxpolicy-core` gains the structured findings behind a rejection (today it
  carries only a summary string), so each finding keeps its code. The controller ignores the new
  field.
- The `notaio` crate gains a library target (the lint engine and the CLI definition, so they are
  testable) and one dependency, `clap` (MIT OR Apache-2.0), which must pass `deny.toml`.
  `sandboxpolicy-types` and `sandboxpolicy-bundle` are unaffected.
- Changing the deployment's arguments is a rollout: a controller image with this change needs
  `args: ["controller"]`.
- The approved-ceiling check (ROADMAP M1) builds on this command rather than adding another.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
