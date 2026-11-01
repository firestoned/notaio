# Roadmap

Ordered by consequence, like the AgentSandbox security roadmap this project belongs to. The
contracts (types, bundle, version rules) have to be stable before Roadmap A phase 1 of the threat
model finishes, because the attestation chain delivers a bundle.

## Status board

One row per roadmap; the detail is in `.github/community/`. Update the row in the same pull
request that changes a roadmap's state.

| # | Roadmap | Status | Detail |
| --- | --- | --- | --- |
| 00 | M3: integrate the consumers | In progress: phase 0 done (v0.3.0); phase 1 under way in mediatore (its roadmap 09) | [00-m3-integrate-the-consumers.md](https://github.com/firestoned/notaio/blob/main/.github/community/00-m3-integrate-the-consumers.md) |
| 01 | Run notaio for real | Done: phases A to D on the test cluster (2026-10-31) | [01-run-notaio-for-real.md](https://github.com/firestoned/notaio/blob/main/.github/community/01-run-notaio-for-real.md) |
| 02 | OpenBao unseals itself | In progress: the static seal decided, phase A proven on kind; the test cluster next | [02-unseal-the-signer.md](https://github.com/firestoned/notaio/blob/main/.github/community/02-unseal-the-signer.md) |
| 03 | Policies delivered as signed OCI artifacts | Not started | [03-signed-policy-artifacts.md](https://github.com/firestoned/notaio/blob/main/.github/community/03-signed-policy-artifacts.md) |

## M0: scaffold and contracts (this repository state)

Done: types, CRDs, knob register and built-in profiles transcribed from the threat model, lint rules
L001 to L013, `evaluate`, bundle format with DSSE signing and verification, controller skeleton,
generated manifests, raw deployment manifests, ADR-0001 and ADR-0002, 39 tests.

Not done: nothing has run on a cluster.

## M1: make the contracts trustworthy

- Run the controller on a k0s test cluster end to end: CRDs, profiles, a policy, a grant, a bundle
  published and verified by a small consumer. Fix what breaks. Try the admission policy there too.
  **Done 2026-10-14** on a three-node k0s cluster (Kubernetes 1.36, kube-router), image pulled from
  the registry by digest: all four e2e suites pass, `examples/` pass a server-side dry run, and the
  controller's NetworkPolicy is enforced (a listener carrying the controller's label is unreachable
  from another namespace, the same listener without it is reachable). Nothing broke. Two follow-ups:
  - The CRDs carry `format: uint8/16/32/64` (schemars, for unsigned Rust types). Kubernetes 1.36
    warns `unrecognized format` on every apply; kind 1.31 does not. Validation is unaffected, since
    unknown formats are ignored. **Fixed 2026-10-15:** `crdgen` maps them to `int32`/`int64`, and a
    test fails on any format Kubernetes does not recognise.
  - Deleting a grant can queue a reconcile of a policy already deleted, which kube-runtime logs at
    WARN ("not found in local store"). Noise, not a failure.
- `notaio lint` CLI that runs the same `sandboxpolicy-core` on files, for pull request checks. This
  is the threat model's "toxic-combination lint on every policy pull request". **Done 2026-10-20**
  (ADR-0006): `notaio lint`, text and SARIF, input codes I001 to I007; the controller is now
  `notaio controller`.
- Approved-ceiling check: a way to record the approved maximum exposure of a profile or policy and
  fail a change that exceeds it (the "maximum exposure compared with its approved ceiling" test).
  **Done 2026-10-20** (ADR-0007): an `ExposureCeiling` per policy, approved by `Security`, checked
  by `notaio lint` and by the controller (L014 to L016, I008). No ceiling, no bundle.
- Decide ADR-0002 open decisions 1, 2 and 4 (distribution, signing key custody, canonicalisation),
  then freeze bundle schema v1. **Decided 2026-10-16:** ADR-0003 (ConfigMap, relayed by mediatore),
  ADR-0004 (OpenBao transit), ADR-0005 (RFC 8785, which bumps the schema to v2). **Done
  2026-10-20:** RFC 8785 and the 2^53 - 1 integer bound implemented, schema v2 frozen (ADR-0002).
  The OpenBao signer is M2; the relay is M3.
- Example verifier for mediatore and mediatore-guest, with persisted "highest version accepted".
  **Done 2026-10-21** (ADR-0008): `sandboxpolicy_bundle::floor` (`accept`, `FileFloor`), keyed by
  namespace and name with the incarnation's `uid` ordered by `issuedAt`, and the
  `verify_and_record` example. **M1 is complete.**

## M2: production controller

- Leader election and more than one replica. **Done 2026-10-26** (ADR-0011): two replicas, a Lease
  in the controller's namespace, our own loop with a pure `decide`; the leader exits when it cannot
  renew, and releases on a clean shutdown; `e2e_leader` deletes the leader and checks the other
  takes over and publishes.
- Health endpoints, Prometheus metrics (relaxations by age and past expiry, lint failures, bundle
  age, fail-open counters that must be zero), Kubernetes Events. **Done 2026-10-23** (ADR-0010):
  one listener on 8080 serving `/healthz`, `/readyz` and `/metrics` and nothing else, ingress from
  the monitoring namespace only; gauges computed from the caches at scrape time; a fail-open guard
  before signing; Events on transitions, with `create`/`patch` on `events.k8s.io` Events added to
  the ClusterRole.
- OpenBao transit `Signer` (ADR-0004): Kubernetes auth bound to notaio's ServiceAccount, a sign-only
  OpenBao policy, one egress rule. Remove the development signer path from the default deployment.
  **Done 2026-10-23:** the signer (`crates/notaio/src/openbao.rs`, 2026-10-22) is the default in
  `deploy/controller/`, every e2e suite runs with it, and the development seed is an opt-in patch
  (`deploy/signer/development-patch.yaml`).
- Image build, signing and provenance. Pin the deployment by digest. **Done 2026-10-23**
  (ADR-0009): multi-arch distroless image from native-runner builds, keyless Cosign by digest,
  SBOM, build provenance; each release attaches `notaio-<version>.yaml` pinned by digest and
  signed. Grype scanning and VEX: **done 2026-10-29** (ADR-0012), with an SBOM that covers every
  Rust crate and VEX statements re-checked against the binary on every pull request.
- Pin the transit key version the controller signs with, so a rotation can be staged at every
  consumer before notaio starts using the new version. **Done 2026-10-30** (ADR-0004, amended):
  `NOTAIO_OPENBAO_KEY_VERSION`, sent with every sign request, a signature from any other version
  refused; `e2e_openbao` pins a version OpenBao lacks (refused) and the one it has (used).

## M3: integrate the consumers

Planned in [roadmap 00](https://github.com/firestoned/notaio/blob/main/.github/community/00-m3-integrate-the-consumers.md): six decisions first
(how consumers get the crates, bundle schema v3 for mediatore's egress model, where the policy is
named, how the guest gets the bundle and the keys, whether the gateway verifies, what a token
carries), then four phases.

**Phase 0, notaio's contract, is done: v0.3.0** (2026-10-31). D1 (ADR-0014: consumers pin a release
tag; the bundle crate's dependencies are kept small in CI) and D2 (ADR-0013: bundle v3 with
endpoints and rules per audience, API v1alpha2) are decided and built, and the conformance kit
says what every consumer must accept and refuse. Next is phase 1, in mediatore: read and verify
the bundles in shadow. D3 to D6 are open.

Running notaio for real, before any consumer reads it, is planned in
[roadmap 01](https://github.com/firestoned/notaio/blob/main/.github/community/01-run-notaio-for-real.md):
installed from the signed release on a long-lived test cluster, with Flux, a private policy
repository, GitOps-only admission, a read-only `e2e_smoke` suite, and a policy change proven from
pull request to verified bundle. **Stage 1 is done** (2026-10-31): a tightening went from pull request to
a verified bundle and a moved floor, a widening past the ceiling failed `L014`, and a sealed
OpenBao stopped signing without breaking the published bundle, recovering on unseal.

That recovery needs a person. [Roadmap 02](https://github.com/firestoned/notaio/blob/main/.github/community/02-unseal-the-signer.md)
makes OpenBao unseal itself (recommended: the `static` seal, its key in a Secret) before a consumer
enforces bundles signed there.

[Roadmap 03](https://github.com/firestoned/notaio/blob/main/.github/community/03-signed-policy-artifacts.md)
delivers the policy repository as signed OCI artifacts that Flux verifies before applying:
`policies` signed by CI after lint, `ceilings` signed by Security alone. It closes the free-plan
review gap where it matters, at the ceilings; notaio's bundle signature stays, since it covers what
notaio computes in the cluster.

- AgentSandbox (mediatore, since mediatore ADR-0008 took the claim from banlieue): names a policy,
  admission checks eligibility with it as the parameter, the claim records the policy version. No
  policy logic in banlieue or mediatore beyond verifying and enforcing the bundle.
- Mediatore: verify the bundle, evaluate at every mint, stamp the version into tokens, refuse stale
  versions.
- Mediatore-guest: render the jail, nftables and managed settings from the verified bundle.
- Gateway: allowlist from the same bundle.

## M4: tighten and extend

- Tightening latency (ADR-0002, decision 3): version floor pushed to mediatore, shorter lifetime.
- Audience classification, so "Lab audiences only" becomes enforceable.
- K2.4 resource sizes once the model defines them.
- Cedar: compile a Cedar policy set into the bundle when the declarative form runs out.
- Knob dashboard over `KnobGrant`.

## M5: public edition

A separate, status-labelled edition of the docs (implemented, planned, proposed), a SECURITY.md, and
a licence review. Confirm employer sign-off first.

Done: `SECURITY.md` (private vulnerability reporting), a MkDocs site (`make docs`, published to
GitHub Pages from `main`) that pulls in the canonical docs, and the Apache-2.0 `LICENSE` with SPDX
headers on every source file, checked in CI. Still open: status labels on every page, employer
sign-off.
