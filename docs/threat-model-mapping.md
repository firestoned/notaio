# Mapping to the AgentSandbox threat model

Last full pass 2026-10-31, against ADR-0001 to ADR-0013.

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
| Egress default deny, no direct-IP egress (K4.1, T4.1) | lint L007 on direct egress; every audience endpoint is an exact name, never a wildcard or IP literal (L017), reached through the broker's proxy, so K4.1 R0 still means all calls are brokered; an endpoint is reachable one way only (L018). ADR-0013. |
| Tool and MCP pinning (K1.3, T1.2) | lint L008 |
| Lease and egress volume ceilings (K7.3, K4.3) | lint L009, `ceilings.rs` |
| O3 "maximum exposure is computable from its policy" | `status.maxExposure` |
| Validation "maximum exposure compared with its approved ceiling" on every policy pull request | `ExposureCeiling` per policy, approved by `Security` (ADR-0007); `lint_exposure` (L014 to L016) runs in `notaio lint` and in the controller, grants included. No ceiling, or exposure it does not cover: no bundle (`e2e_bundle`). |
| T5.4 bundle downgrade or tampering | signed DSSE envelope, monotonic version, verification refuses a downgrade, a forged signature and a digest mismatch (`crates/sandboxpolicy-bundle/src/verify.rs`). The digest covers the RFC 8785 form of the content (`jcs.rs`, ADR-0005), so a consumer in any language can recompute it; content with no exact canonical form is refused, not approximated. The version floor a consumer keeps is persisted before a bundle is acted on, so a restart does not reopen the downgrade, and it is keyed by namespace and name with the incarnation's `uid` ordered by `issuedAt`, so a recreated policy is accepted and a replay from its old incarnation is refused (`floor.rs`, ADR-0008). `e2e_bundle` checks a wrong key and a downgrade across a consumer restart against the deployed controller. |
| T8.2 signing key not held by administrators | The default deployment (`deploy/controller/`) signs with a non-exportable ed25519 OpenBao transit key (ADR-0004, `crates/notaio/src/openbao.rs`): notaio logs in with its ServiceAccount (projected token, audience `openbao`, TLS 1.3 against OpenBao's own CA) and may only request signatures; every request is in OpenBao's audit log. The controller refuses to start with both signers configured; the development seed is an opt-in patch for local experiments, used by no e2e suite. Accepted risk: a cluster administrator acting as notaio's ServiceAccount can request signatures, but cannot take the key. `e2e_openbao`. |
| Hard floor "policy bundles are signed, unsigned ones are rejected" | producer never publishes unsigned (`crates/notaio/src/reconcile.rs`: no signer, no bundle), consumer verification fails closed. Content without a canonical form is never compiled into a bundle (`evaluate`, `Ready=False, NotCanonical`). When the signer fails (OpenBao sealed or unreachable), nothing new is published and the policy reports `Ready=False, SigningFailed`, keeping the previous bundle's version and digest (`e2e_openbao`). |
| The integrity of notaio's own image, the component that signs policy | Built only by this repository's CI on native runners, keyless Cosign signature by digest, CycloneDX SBOM, build provenance (`actions/attest-build-provenance`); each release attaches its install manifest pinned to the image digest and signed (ADR-0009, `.github/workflows/build.yaml`). The release logic is tested on every pull request (`release-manifest-test`, `set-version-test`). |
| Metric "relaxations past their expiry: 0" and the relaxation dashboard | `notaio_relaxations_past_expiry` (active grants past `expiresAt`, must be 0), and per active grant `notaio_relaxation_age_seconds` and `notaio_relaxation_expires_in_seconds`, served on `/metrics` (ADR-0010, `crates/notaio/src/metrics.rs`). `e2e_install` reads the deployed value. The dashboard itself belongs to the site's Prometheus. |
| A fail-open bug in the evaluator | a guard independent of `evaluate` refuses to sign a bundle without a ceiling, with an error finding, with an expired relaxation, or already past `notAfter`; `Ready=False, FailOpenRefused` and `notaio_fail_open_total`, which must be 0 (ADR-0010, `crates/notaio/src/guard.rs`). |
| Known vulnerabilities in notaio's image | Grype scans every published image (ADR-0012), its SBOM covering every Rust crate (`cargo auditable`), findings in Code Scanning, the build failing on a fixable High or Critical. Findings that cannot affect notaio are suppressed only by OpenVEX statements in `.vex/`, each backed by a fact about the binary that `make vex-evidence` re-checks on every pull request; the VEX document is attested to the image and attached to each release. cargo-audit and cargo-deny cover the Rust dependencies on every change. |
| Policy profiles as reviewed bundles of knobs (section 10) | `builtin.rs`, `deploy/profiles/builtin.yaml` |

## Supported, enforced elsewhere

| Model item | Where |
| --- | --- |
| T8.2 silent edit of a policy | `deploy/admission/gitops-only.yaml` here (ExposureCeiling included), tested by `e2e_admission` on kind and k0s, plus review in the policy repository. The two-person review rule is a repository setting. The controller cannot edit policy objects either (`10-rbac.yaml`, `e2e_rbac`). |
| Profile eligibility by group (K7.2, T7.3) | Banlieue admission, using these objects as parameters |
| Enforcement of egress, tools, leases, budgets, jail and guard settings | Gateway, mediatore, mediatore-guest, from the bundle. Distribution (ADR-0003): mediatore reads the bundle ConfigMap through the API and relays it unchanged to guests, which verify it themselves. Not built yet (ROADMAP M3). |
| Token minting per policy and audience (T5.3) | Mediatore |
| Downgrade protection at each consumer (T5.4) | mediatore, mediatore-guest and the gateway call `sandboxpolicy_bundle::floor::accept` with a floor on their own storage (ADR-0008). Not adopted yet (ROADMAP M3). |

## Not covered yet

| Model item | Gap |
| --- | --- |
| Lab audiences only (K5.2, K5.3 for Lab) | Needs an audience classification |
| Knob prerequisites proven by passing control tests | Grants record approvals, not test results |
