# ADR-0007: ExposureCeiling, the approved maximum exposure of a policy

- **Status:** Accepted
- **Date:** 2026-10-20
- **Deciders:** Erick Bourgeois
- **Related:** ROADMAP M1 ("approved-ceiling check"); threat model objective O3 ("maximum exposure
  is computable from its policy") and the validation "maximum exposure compared with its approved
  ceiling"; [ADR-0006](0006-notaio-lint-cli.md) (`notaio lint`); T8.2

Implemented with this ADR: `crates/sandboxpolicy-api/src/ceiling.rs`, `lint_exposure` in
`crates/sandboxpolicy-core/src/lint.rs`, the gate in `evaluate`, the controller's fetch and watch,
`notaio lint`, `deploy/` (CRD, RBAC, admission) and `examples/exposureceiling-agent-default.yaml`.

## Context

notaio already computes what a fully compromised sandbox could reach, `status.maxExposure`:
audiences with their access and token lifetime, egress hosts, tools, and the lease ceiling. It
includes whatever active grants add. Nothing compares it with anything. A pull request can widen a
policy, or add a grant that widens it, and as long as every lint rule passes, it publishes.

The threat model wants each policy's exposure compared with an approved ceiling, so that widening
it is a separate, deliberate approval, not a side effect of a change that passed lint.

## Decision

1. **A fourth kind, `ExposureCeiling`** (`sandbox.firestoned.io/v1alpha1`, namespaced), named
   after the policy it caps: one ceiling per policy, unique by construction. Its spec mirrors
   `maxExposure`:

   ```yaml
   spec:
     audiences:          # each with the most access and the longest token it may have
       - name: internal-git
         access: Read
         maxTtlSeconds: 300
     egressHosts: []     # "host:port"
     tools: []           # tool names
     maxLeaseSeconds: 14400
     approvals:
       - role: Security
         reference: https://example.com/review/1
   ```

2. **A policy's exposure must be covered by its ceiling.** Every exposed audience is in the
   ceiling with no more access (Read below Write) and no longer a token; every egress host and tool
   is in the ceiling; the lease is no longer. The comparison is on the compiled content, so active
   grants count.
3. **No ceiling, no bundle.** A policy without an `ExposureCeiling` fails, at pull request and in
   the cluster. So does one whose exposure the ceiling does not cover, and a ceiling without an
   approval from the `Security` role.
4. **Enforced twice, with the same code.** `notaio lint` compares at pull request time. The
   controller compares on every reconcile and, on a failure, publishes nothing new and reports
   `Ready=False`. `status.maxExposure` is still reported, so approvers can see exactly what to
   approve. A check skipped in the repository, or a change that reached the cluster another way,
   still cannot publish.
5. **New stable codes:**

   | Code | Meaning |
   | ---- | ------- |
   | L014 | The policy's exposure is not covered by its approved ceiling |
   | L015 | The policy has no approved ceiling |
   | L016 | The ceiling lacks an approval from the `Security` role |
   | I008 | An ExposureCeiling names a policy that is not in the inputs (`notaio lint`) |

   Each has a test that fails when its check is removed.
6. **Ceilings change only through GitOps,** like the other kinds:
   `deploy/admission/gitops-only.yaml` matches `exposureceilings`. The controller reads them and
   never writes them. Its RBAC gains `get`, `list` and `watch` on `exposureceilings` only.

## Consequences

- Widening a policy now takes two approvals: the policy change itself, and the ceiling change that
  covers it. A policy repository puts ceilings under a path its CODEOWNERS gives to the security
  approvers, so the second approval is a different person.
- A KnobGrant that widens a policy needs a ceiling that already covers the wider exposure, so a
  time-boxed relaxation cannot quietly exceed what Security approved.
- **Breaking for a running installation:** after this lands, every policy needs a ceiling before it
  publishes again. Consumers keep the last good bundle until it expires, so record the ceilings
  before rolling out the controller.
- The `Security` approval requirement is this ADR's choice. If the threat model names a different
  approver for exposure, the rule changes with it.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
