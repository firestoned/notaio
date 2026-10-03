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
| The controller cannot create, edit or delete a policy object (T8.2) | `deploy/controller/10-rbac.yaml`: read plus status writes | `e2e_rbac`, through SubjectAccessReview against the deployed role |
| No direct edits, only reviewed GitOps changes (T8.2, S6) | `deploy/admission/gitops-only.yaml` | `e2e_admission`: denied for cluster-admin, admitted for the GitOps identity |
| The controller reads no Secrets through the API, and writes bundles only where allowed | per-namespace Role, no Secret verbs | `e2e_rbac` |
| No inbound connections; egress to DNS and the API server only | `deploy/controller/30-networkpolicy.yaml` | applied in every e2e run |
| Toxic combinations, wildcard audiences, multi-step grants are refused | lint rules L001 to L013 | `sandboxpolicy-core` tests |

## Supply chain

- Every GitHub Action is pinned by commit SHA; container images by digest.
- `cargo-deny` (licences, advisories, sources) and `cargo-audit` on every change.
- CodeQL (Rust, Python, Actions) and Semgrep on every pull request and push.
- OpenSSF Scorecard, weekly and on every push to `main`.
- Dependabot with a 7 day cooldown; updates merge only after the e2e passes.
- Every commit is signed and verified in CI.

Image signing, SBOMs and build provenance arrive with the published image (ROADMAP M2).

## Known gaps

The development signer keeps its key in a Kubernetes Secret, which the threat model forbids for
production (T8.2). A KMS, HSM or TPM backed signer is ADR-0002 open decision 2. The other open items
are listed under "Known gaps in v0" in the [Architecture](../architecture.md#known-gaps-in-v0).
