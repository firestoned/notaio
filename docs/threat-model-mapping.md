# Mapping to the AgentSandbox threat model

Last full pass 2026-10-20, against ADR-0001 … ADR-0007.

What this project implements, what it only supports, and what it does not cover. Identifiers refer
to the AgentSandbox Threat Model and Security Roadmap. When that document changes, update
`crates/sandboxpolicy-core/src/registry.rs` and `builtin.rs` in the same pull request.

## Implemented here

| Model item | How |
| --- | --- |
| Knob register (section 9), rule "one step at a time" | `registry.rs`, lint L011 |
| Expiring relaxations, per-step lifetime caps | lint L012, bundle `notAfter` clamped to the earliest grant expiry |
| Approver per knob | `registry.rs` approvers, lint L013 |
| "No toxic combinations" | lint L003, with `dataScope` for Lab |
| "Toxic-combination lint on every policy pull request" | `notaio lint` (`crates/notaio/src/lint.rs`, ADR-0006) runs the controller's own checks on the policy repository's files, with the same codes, and SARIF for inline review. Making it a required check is a setting of the policy repository. |
| Lab-only knobs (K6.3 R1, K8.2 R1) | lint L004 |
| "Wildcard audiences are never allowed" (K5.3) | lint L005 |
| Read-only token scope by default (K5.2), token TTL ceilings (K5.1) | lint L006 |
| Egress default deny, no direct-IP egress (K4.1, T4.1) | lint L007 |
| Tool and MCP pinning (K1.3, T1.2) | lint L008 |
| Lease and egress volume ceilings (K7.3, K4.3) | lint L009, `ceilings.rs` |
| O3 "maximum exposure is computable from its policy" | `status.maxExposure` |
| Validation "maximum exposure compared with its approved ceiling" on every policy pull request | `ExposureCeiling` per policy, approved by `Security` (ADR-0007); `lint_exposure` (L014 to L016) runs in `notaio lint` and in the controller, grants included. No ceiling, or exposure it does not cover: no bundle (`e2e_bundle`). |
| T5.4 bundle downgrade or tampering | signed DSSE envelope, monotonic version, verification refuses a downgrade, a forged signature and a digest mismatch (`crates/sandboxpolicy-bundle/src/verify.rs`). The digest covers the RFC 8785 form of the content (`jcs.rs`, ADR-0005), so a consumer in any language can recompute it; content with no exact canonical form is refused, not approximated. `e2e_bundle` checks a wrong key and a downgrade against the deployed controller. |
| Hard floor "policy bundles are signed, unsigned ones are rejected" | producer never publishes unsigned (`crates/notaio/src/reconcile.rs`: no signer, no bundle), consumer verification fails closed. Content without a canonical form is never compiled into a bundle (`evaluate`, `Ready=False, NotCanonical`). |
| Policy profiles as reviewed bundles of knobs (section 10) | `builtin.rs`, `deploy/profiles/builtin.yaml` |

## Supported, enforced elsewhere

| Model item | Where |
| --- | --- |
| T8.2 silent edit of a policy | `deploy/admission/gitops-only.yaml` here (ExposureCeiling included), tested by `e2e_admission` on kind and k0s, plus review in the policy repository. The two-person review rule is a repository setting. The controller cannot edit policy objects either (`10-rbac.yaml`, `e2e_rbac`). |
| Profile eligibility by group (K7.2, T7.3) | Banlieue admission, using these objects as parameters |
| Enforcement of egress, tools, leases, budgets, jail and guard settings | Gateway, mediatore, mediatore-guest, from the bundle. Distribution (ADR-0003): mediatore reads the bundle ConfigMap through the API and relays it unchanged to guests, which verify it themselves. Not built yet (ROADMAP M3). |
| Token minting per policy and audience (T5.3) | Mediatore |

## Not covered yet

| Model item | Gap |
| --- | --- |
| T8.2 signing key not held by administrators | Development signer only (a seed in a Secret). Decided, not built: a non-exportable OpenBao transit key (ADR-0004, ROADMAP M2). Accepted risk once built: a cluster administrator who can act as notaio's ServiceAccount can request signatures, audited outside the cluster, but cannot take the key. |
| Metric "relaxations past their expiry: 0" and the relaxation dashboard | No metrics endpoint yet (ROADMAP M2). The data is in `KnobGrant` status. |
| Lab audiences only (K5.2, K5.3 for Lab) | Needs an audience classification |
| Knob prerequisites proven by passing control tests | Grants record approvals, not test results |
