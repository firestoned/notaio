# Roadmap 01: run notaio for real

- **Status:** Phases A to D done (2026-10-31): notaio v0.3.1 signs through OpenBao, policies
  reach the cluster only from the private policy repository through Flux, the smoke test passes,
  and the loop held for a tightening, a widening and a sealed signer. Phase A's follow-up is done
  and ships in the next release.
- **Milestone:** M3 in [`ROADMAPS.md`](../../ROADMAPS.md), stage 1 of running it end to end
- **Related:** roadmap 00 (M3), whose phase 1 reads what this publishes; ADR-0004 (the signer),
  ADR-0009 (releases), ADR-0010 (health and metrics), ADR-0011 (leader election)

## 1. Goal

Run notaio on a long-lived test cluster the way an operator would run it in production: installed
from a signed release, signing with OpenBao, with policies that arrive only from a private policy
repository through GitOps, and checked by a test that changes nothing. Then a policy change made by
pull request becomes a signed bundle a consumer can verify, with every gate on the way exercised.

This is stage 1. Stage 2, a real sandbox whose tokens and jail follow the policy, needs mediatore
to read and enforce the bundle: roadmap 00, phases 1 to 3.

## 2. Where things stand

| Piece | State |
| --- | --- |
| Kubernetes | a three-node test cluster with cert-manager, SPIRE and banlieue; no GitOps reconciler |
| OpenBao | the `dev/openbao` test instance (PostgreSQL storage, one key share): unsealed by hand, and sealed again by any restart |
| notaio | not installed; no CRDs |
| mediatore | not installed |
| Prometheus | not installed |

## 3. Decisions

- **GitOps: Flux, managed by the Flux Operator** (a `FluxInstance` installs and upgrades Flux). The
  Flux Operator is AGPL-3.0 and single-vendor (ControlPlane), which this project's dependency rule
  screens out; it is used here anyway, on purpose, because it is a tool on a test cluster, run
  unmodified, and not a notaio dependency. notaio requires neither it nor any GitOps tool, and its
  documented path stays plain Flux (Apache-2.0, CNCF). Flux's `kustomize-controller` becomes the
  only identity the GitOps-only admission policy admits.
- **Policies live in a new private repository.** Real policies name real hosts, which never go in a
  public repository. Its CI runs `notaio lint` from a pinned release; CODEOWNERS gives `ceilings/`
  to the security approvers.
- **OpenBao stays the test instance,** unsealed by hand. A restart stops signing until it is
  unsealed; policies report `SigningFailed` and bundles already published stay valid until their
  `notAfter`. Auto-unseal is a separate decision, before anything depends on this cluster.
- **The real cluster gets a read-only test.** The kind suites create and delete policy objects as
  cluster-admin, which GitOps-only admission rightly refuses; exempting a namespace would weaken the
  T8.2 control on the one cluster meant to show it working. Those suites stay on kind. The real
  cluster gets `e2e_smoke`, which changes nothing.

## 4. Work, in order

### Phase A: install notaio from the release

- [x] A server-side dry-run of the v0.3.0 manifest against the cluster, before installing it,
      found two install defects: the built-in profiles fail a first apply in the same file as their
      CRDs, and the manifest carried a bundle Role for an example namespace. Fixed in v0.3.1: the
      CRDs ship separately, and the Role is a per-namespace template (ADR-0009, amended).
- [x] Verify both manifests as the install guide says: checksums, Cosign signatures, provenance
      (v0.3.1, 2026-10-31).
- [x] `notaio-system` with the OpenBao CA in ConfigMap `openbao-ca`; the NetworkPolicy's API
      server rule narrowed to the API server's endpoints, in a local copy of the manifest only.
- [x] Apply the CRDs, wait, apply the rest; `deploy/policy-namespace/` in the first policy
      namespace. Two replicas on different nodes, one Lease holder, the built-in profiles `Valid`
      (which also shows the narrowed NetworkPolicy still reaches the API server), `/readyz` ready,
      both must-be-zero series at 0. The controller may write ConfigMaps in the policy namespace
      and nowhere else, and may not edit a policy.
- [x] One real signature, before admission is on: the example policy and ceiling, applied briefly,
      published a v3 bundle signed by the transit key, with a `Published` Event; the consumer
      example accepted it against OpenBao's public key and refused it against another key; both
      objects deleted, their bundle with them.
- [x] Follow-up: a watch that expires ("too old resource version") is logged as a `WARN` "policy
      controller error", though the watcher re-lists by itself. Log it quietly. Now at `debug`,
      with the other controller errors that need no attention; anything else still warns.

### Phase B: GitOps and the policy repository

- [x] The Flux Operator v0.62.1 (`install.yaml` checked against the release's checksums, the
      operator image's Cosign signature verified) and a `FluxInstance` for Flux v2.10.0, syncing
      the policy repository's `clusters/core` with a read-only deploy key the maintainer generated,
      and GitHub's host keys checked against the ones GitHub publishes. `platform` and
      `ns-sandboxes` Kustomizations, the second including notaio's bundle Role.
- [x] The policy repository: `namespaces/<namespace>/` holding its namespace, notaio's bundle Role,
      `policies/` and `ceilings/`, with CODEOWNERS on `ceilings/`; the `notaio lint` workflow from
      the lint guide, pinned to the release image by digest, in text mode (SARIF upload to Code
      Scanning needs GitHub Advanced Security on a private repository). Bootstrapped by its first
      pull request.
- [x] The identity provider emits groups: the GitHub team `firestoned:sandbox-users`, and Dex's
      GitHub connector with `loadAllGroups` (no restriction on who may log in) and team slugs.
- [x] The first real policy: the research agent from mediatore's walkthrough, as v1alpha2, its
      `github` audience reached brokered at GitHub's API with read and write-contents rules, and
      its ExposureCeiling. Applied by Flux, published as bundle version 1 signed by the transit
      key, and verified against OpenBao's public key with the consumer example.
- [x] GitOps-only admission, naming Flux's `kustomize-controller`, applied by Flux. A direct edit
      of the policy as cluster-admin is denied.

### Phase C: the read-only smoke test

- [x] `e2e_smoke` (new suite in notaio), run with `make cluster-smoke` against any cluster:
      the controller is healthy and leads once; every SandboxPolicy is `Ready=True, Published`; every
      published bundle verifies against the OpenBao public key and records a floor; both
      must-be-zero series are 0; a direct edit of a policy object is denied. It creates nothing and
      deletes nothing.
- [x] Passes 3 of 3 on the test cluster. Shown to catch what it guards: on a cluster without the
      admission policy, `a_direct_edit_is_denied` fails.
- [x] Run after every notaio release is installed, and after every policy repository merge. The
      commands and every run are kept in the policy repository's `TESTING.md`.

### Phase D: prove the loop

- [x] A pull request to the policy repository changes one rule: `notaio lint` passes in CI, review
      approves, the merge is applied by Flux, notaio publishes the next version with a `Published`
      Event, and the bundle verifies with the consumer example and the floor moves. The lease cap
      went from 3600 s to 1800 s: version 2, verified, the floor moved from 1 to 2.
- [x] A pull request that widens an endpoint without widening the ceiling fails `notaio lint` with
      `L014` before it can merge. It did, and nothing reached the cluster. GitHub still offered the
      merge button: on the free plan a failing check cannot block a merge (section 6), so the
      cluster's own ceiling check is what stops such a merge from being signed.
- [x] OpenBao sealed on purpose: the policy reports `SigningFailed`, the bundle already published
      keeps verifying until its `notAfter`, and unsealing recovers without touching notaio. Sealed,
      then a tightening merged: `Ready=False, SigningFailed` with a Warning Event and
      `notaio_signing_failures_total` counting, version 2 still verifying. Unsealed: version 3
      within 10 s, no restart, the floor moved to 3 and a replay of version 2 was refused.

## 5. Done when

Phase D's three checks pass on the test cluster, `e2e_smoke` passes after a release install and
after a policy merge, and the install guide was followed as written, with any step that did not
work fixed in the guide.

## 6. Risks

- **Review is not enforced on GitHub's free plan.** Branch protection and rulesets on a private
  repository need a paid plan, so someone with push access can commit straight to `main`; CODEOWNERS
  and the lint check show on pull requests but cannot block a merge. What still holds: nobody can
  edit a policy on the cluster directly (GitOps-only admission), and notaio refuses any policy that
  breaks a rule or exceeds its ceiling, however it reached `main`. Acceptable for a
  single-maintainer test cluster; anything others depend on needs a paid plan or another host.

- **The test OpenBao seals on every restart.** Fine for stage 1; a cluster others depend on needs
  auto-unseal first: roadmap 02.
- **No Prometheus.** The smoke test reads `/metrics` directly; alerting is not exercised until a
  monitoring stack is installed.
- **The private repository's deploy key** is a credential in the cluster. Read-only, one
  repository, rotated with the repository's settings.
