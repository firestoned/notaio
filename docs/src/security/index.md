# Security

notaio sits on the trust path of every AgentSandbox VM: mediatore and mediatore-guest enforce what
its bundles say. Its security posture is therefore part of the platform's, and it is defined by the
**AgentSandbox Threat Model and Security Roadmap**, not by this repository.
[Threat Model Mapping](threat-model-mapping.md) states which items notaio implements, which it
supports for another component to enforce, and which it does not cover.

## Reporting a vulnerability

**Do not open a public issue.** Report it privately through
[GitHub private vulnerability reporting](https://github.com/firestoned/notaio/security/advisories/new).
The full policy, including response times and scope, is in
[SECURITY.md](https://github.com/firestoned/notaio/blob/main/SECURITY.md).

## The guarantees, and what checks them

| Guarantee | Enforced by | Checked by |
| --- | --- | --- |
| No unsigned bundle is ever published | the controller has no unsigned publish path; with no key it reports `Ready=False, NoSigner` | unit tests; `e2e_bundle` verifies every published bundle |
| A consumer refuses a forged, expired or downgraded bundle (T5.4) | `verify_bundle`, failing closed on every check | `sandboxpolicy-bundle` tests; `e2e_bundle` (wrong key, version floor) |
| The controller cannot create, edit or delete a policy object (T8.2) | `deploy/controller/10-rbac.yaml`: read plus status writes, `create`/`patch` on Events ([ADR-0010](../adr/0010-health-metrics-and-events.md)), and its own Lease in its own namespace ([ADR-0011](../adr/0011-leader-election.md)) | `e2e_rbac`, through SubjectAccessReview against the deployed role |
| No direct edits, only reviewed GitOps changes (T8.2, S6) | `deploy/admission/gitops-only.yaml` | `e2e_admission`: denied for cluster-admin, admitted for the GitOps identity |
| The controller reads no Secrets through the API, and writes bundles only where allowed | per-namespace Role, no Secret verbs | `e2e_rbac` |
| Known vulnerabilities in the image are found, fixed or accounted for | Grype on every published image, failing on fixable High and Critical; VEX statements in `.vex/` only with evidence about the binary ([ADR-0012](../adr/0012-image-scanning-and-vex.md)) | `make vex-evidence` on every pull request, for both architectures; findings in Code Scanning |
| No inbound connection that can influence signing; egress to DNS, the API server and OpenBao only | `deploy/controller/30-networkpolicy.yaml`: ingress only to the health and metrics port, from the monitoring namespace; the listener serves three fixed `GET` paths and reads no body (ADR-0010) | applied in every e2e run; `crates/notaio/src/http_tests.rs` |
| A bundle the evaluator should never have produced is not signed | the fail-open guard (`crates/notaio/src/guard.rs`), counted in `notaio_fail_open_total`, which must be 0 | unit tests; `e2e_install` reads the deployed counters |
| Toxic combinations, wildcard audiences, multi-step grants are refused | lint rules L001 to L013 | `sandboxpolicy-core` tests |
| No policy publishes past its approved exposure, and none publishes without one | ExposureCeiling and lint rules L014 to L016 ([ADR-0007](../adr/0007-exposure-ceiling.md)), checked by `notaio lint` and by the controller | `sandboxpolicy-core` tests; `e2e_bundle` (no ceiling, no bundle) |

## Supply chain

- Every GitHub Action is pinned by commit SHA; container images by digest.
- `cargo-deny` (licences, advisories, sources) and `cargo-audit` on every change.
- CodeQL (Rust, Python, Actions) and Semgrep on every pull request and push.
- OpenSSF Scorecard, weekly and on every push to `main`.
- Dependabot with a 7 day cooldown; updates merge only after the e2e passes.
- Every commit is signed and verified in CI.

- Every published image, `ghcr.io/firestoned/notaio`, is built by this repository's CI for
  amd64 and arm64, Cosign-signed by digest (keyless), and carries a CycloneDX SBOM and build
  provenance ([ADR-0009](../adr/0009-release-and-supply-chain.md)). A release attaches its install
  manifest with the image pinned to that digest, signed the same way. How to check it yourself:
  the developer guide's "Verifying a release".

## Known gaps

The default deployment signs with [ADR-0004](../adr/0004-signing-key-custody.md)'s
non-exportable OpenBao transit key: notaio may ask for signatures and can never take the key. The
development seed, which anyone who can read its Secret could use, survives only as an opt-in
patch for local experiments (`deploy/signer/development-patch.yaml`). The other open items
are listed under "Known gaps in v0" in the [Architecture](../architecture.md#known-gaps-in-v0).
