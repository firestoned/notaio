# Mapping to the AgentSandbox threat model

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
| Lab-only knobs (K6.3 R1, K8.2 R1) | lint L004 |
| "Wildcard audiences are never allowed" (K5.3) | lint L005 |
| Read-only token scope by default (K5.2), token TTL ceilings (K5.1) | lint L006 |
| Egress default deny, no direct-IP egress (K4.1, T4.1) | lint L007 |
| Tool and MCP pinning (K1.3, T1.2) | lint L008 |
| Lease and egress volume ceilings (K7.3, K4.3) | lint L009, `ceilings.rs` |
| O3 "maximum exposure is computable from its policy" | `status.maxExposure` |
| T5.4 bundle downgrade or tampering | signed envelope, digest, monotonic version, verification refuses downgrade |
| Hard floor "policy bundles are signed, unsigned ones are rejected" | producer never publishes unsigned, consumer verification fails closed |
| Policy profiles as reviewed bundles of knobs (section 10) | `builtin.rs`, `deploy/profiles/builtin.yaml` |

## Supported, enforced elsewhere

| Model item | Where |
| --- | --- |
| T8.2 silent edit of a policy | `deploy/admission/gitops-only.yaml` here, plus review in the policy repository. The two-person review rule is a repository setting. |
| Profile eligibility by group (K7.2, T7.3) | Banlieue admission, using these objects as parameters |
| Enforcement of egress, tools, leases, budgets, jail and guard settings | Gateway, mediatore, mediatore-guest, from the bundle |
| Token minting per policy and audience (T5.3) | Mediatore |

## Not covered yet

| Model item | Gap |
| --- | --- |
| T8.2 signing key not held by administrators | Development signer only (ADR-0002, decision 2) |
| Validation "maximum exposure compared with its approved ceiling" on every policy pull request | No approved-ceiling object and no CLI for PR-time checks yet (ROADMAP M1) |
| Metric "relaxations past their expiry: 0" and the relaxation dashboard | No metrics endpoint yet (ROADMAP M2). The data is in `KnobGrant` status. |
| Lab audiences only (K5.2, K5.3 for Lab) | Needs an audience classification |
| Knob prerequisites proven by passing control tests | Grants record approvals, not test results |
