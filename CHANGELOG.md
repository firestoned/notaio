# Changelog

Notable changes to notaio, by release, in the style of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/). Each release's images and install manifest are signed
and pinned by digest ([ADR-0009](https://github.com/firestoned/notaio/blob/main/docs/adr/0009-release-and-supply-chain.md)).

Add an entry under **Unreleased** in the pull request that makes a change a user would notice. At
a release, the maintainer renames Unreleased to the version and date.

## Unreleased

Nothing yet.

## [0.3.2] - 2026-10-31

What running 0.3.1 on a real cluster showed: a smoke test to run against an installation, and a
routine watch expiry no longer logged as a warning. Nothing about policies, bundles or the API
changes; upgrading is applying the two manifests of this release.

### Added

- `make cluster-smoke`: a read-only smoke test of a running installation (`e2e_smoke`), for
  clusters where the other suites cannot create fixtures: the controller healthy and leading once,
  every policy published with a bundle that verifies, and direct edits denied.

### Changed

- The controller no longer logs an expired watch ("too old resource version") as a `WARN` "policy
  controller error": the watcher re-lists by itself, so it is logged at `debug`. So are a reconcile
  of an object deleted meanwhile, and a failed reconcile, which `reconcile failed, retrying` already
  reports. Other watch errors, such as a 403, still warn.

### Security

- A VEX statement for CVE-2026-84243 (glibc gettext and `LANGUAGE`, published after 0.3.1 was
  built): notaio imports none of the gettext functions, which CI re-checks on every build.

## [0.3.1] - 2026-10-31

Install fixes, found by installing 0.3.0 on a real cluster. Nothing about policies, bundles or the
controller's behaviour changes.

### Upgrading from 0.3.0

Apply `notaio-0.3.1-crds.yaml`, wait for the CRDs, then `notaio-0.3.1.yaml`. If 0.3.0 installed the
`notaio-bundles` Role and RoleBinding in a `sandboxes` namespace you do not use, delete them; in each
namespace you do use, apply `deploy/policy-namespace/notaio-bundles.yaml` from this tag.

### Fixed

- A first install from the release manifest failed on the built-in profiles: they were in the same
  file as the CRDs that define them, and `kubectl` reads every kind before creating any. The CRDs
  now ship as their own asset, `notaio-<version>-crds.yaml`, signed and checksummed like the rest;
  apply it, wait, then `notaio-<version>.yaml`.
- The install manifest no longer carries a bundle Role and RoleBinding for an example namespace,
  `sandboxes`, which every install had to create or edit. The Role is now a template with no
  namespace, `deploy/policy-namespace/notaio-bundles.yaml`, applied in each policy namespace.

## [0.3.0] - 2026-10-31

The contract consumers build on (roadmap 00, phase 0): bundle schema v3 carries each audience's
endpoints and rules in the shape mediatore's egress proxy uses, consumers pin this tag, and a
conformance kit says what every consumer must accept and refuse. **This release is breaking.**

### Upgrading from 0.2

1. Rewrite the policy repository's objects as `sandbox.firestoned.io/v1alpha2`: the `apiVersion`,
   each audience's `endpoints` and `rules`, direct `egress` as `{ endpoint, rules }`, and each
   ceiling audience's `endpoints`. `notaio lint` from this release checks them, and refuses any
   v1alpha1 file with `I009`.
2. Apply this release's `notaio-0.3.0.yaml`, then let GitOps re-apply the objects. v1alpha1 is
   retired, not converted.
3. Bundles are published at schema v3. No consumer read v2 bundles, so none breaks; consumers
   start from this tag.

### Changed

- **Breaking: the API is `sandbox.firestoned.io/v1alpha2`, and v1alpha1 is retired** (ADR-0013).
  Rewrite policy objects as v1alpha2 before upgrading; `notaio lint` refuses a v1alpha1 file with
  `I009`. See the install guide's upgrade note.
- **Breaking: bundle schema v3.** Each audience carries its `endpoints` (`host:port`) and `rules`
  (`{ method, path }`), in the shape mediatore's egress proxy uses; direct egress is
  `{ endpoint, rules }`. The payload type is `...bundle.v3+json`, so a v2 consumer refuses a v3
  bundle rather than misreading it.
- The ExposureCeiling approves endpoints per audience: an audience endpoint it does not list is
  an `L014` finding.

### Added

- A conformance kit: 14 signed bundles, each with the verdict every consumer must reach, and the v3
  payload's JSON Schema, in `crates/sandboxpolicy-bundle/conformance/` and
  `sandboxpolicy_bundle::conformance`. Generated deterministically and checked in CI.
- Consumers depend on a release tag: `sandboxpolicy-bundle = { git = "https://github.com/firestoned/notaio", tag = "v0.3.0" }`
  (ADR-0014). The bundle crate no longer pulls in `schemars`, and CI keeps its dependency tree free
  of Kubernetes, async, networking and TLS.
- Lint rules `L017` (an audience endpoint or rule is malformed) and `L018` (an endpoint is
  reachable two ways).

## [0.2.0] - 2026-10-30

Supply chain and key rotation: every image is scanned and accounted for, its SBOM is complete,
and a signing key rotation can be staged without a gap at any consumer.

### Added

- `NOTAIO_OPENBAO_KEY_VERSION` pins the transit key version notaio signs with, so a key rotation
  can be staged at every consumer before notaio uses the new version. A signature from any other
  version is refused. The rotation guide now stages rotations with it.
- Every published image is scanned with Grype; findings go to Code Scanning, and a fixable High or
  Critical fails the build (ADR-0012).
- OpenVEX statements for the base image findings that cannot affect notaio, each backed by a fact
  about the binary that CI re-checks on every pull request. The VEX document is attested to each
  image and attached to each release as `notaio.vex.json`.

### Changed

- The distroless `cc-debian13` base image is updated to a newer digest.
- Every source file carries an SPDX licence header (`Apache-2.0`), checked in CI.

### Fixed

- The image SBOM lists notaio's Rust crates. In 0.1.0 it held only the base image's packages; the
  binary is now built with `cargo auditable`, and the build fails if the SBOM loses them.
- The install guide says Cosign 3 or later is needed to verify the image signature (Cosign 2
  reports "no signatures found"), and how to run `gh attestation verify` past a stale ghcr login.

## [0.1.0] - 2026-10-26

The first release: the controller, the four policy kinds, signed bundles, and everything an
operator needs to run it with two replicas and an OpenBao signer.

### Added

- **Four policy kinds** in `sandbox.firestoned.io/v1alpha1`: SandboxProfile, SandboxPolicy,
  KnobGrant and ExposureCeiling, with the four built-in profiles of the threat model's profile
  matrix.
- **The controller**: lints every object, applies time-boxed grants, checks each policy against its
  approved ceiling (ADR-0007), compiles a signed, versioned bundle and publishes it as a ConfigMap
  (ADR-0002, ADR-0003). Fails closed at every step.
- **Bundle signing with OpenBao transit**, a non-exportable ed25519 key, as the default signer
  (ADR-0004). A development seed is an opt-in patch for local experiments.
- **The bundle contract**: a DSSE envelope over content digested in its RFC 8785 canonical form
  (ADR-0005), with `verify_bundle` and a persisted consumer version floor (ADR-0008) in
  `sandboxpolicy-bundle`.
- **`notaio lint`** for policy repositories: the controller's own rules on files, with text and
  SARIF output (ADR-0006).
- **Two replicas with leader election** on a Lease: the leader reconciles, signs and publishes,
  and a follower takes over within 15 seconds, or at once on a clean shutdown (ADR-0011).
  `NOTAIO_POD_NAME` is now required.
- **Health, metrics and Events**: `/healthz`, `/readyz` and `/metrics` on port 8080, two
  must-be-zero series, a fail-open guard before signing, and Events on transitions (ADR-0010).
- **Release pipeline**: multi-arch images built on native runners, keyless Cosign signatures,
  SBOMs, build provenance and a digest-pinned install manifest per release (ADR-0009).
- **GitOps-only admission**: a ValidatingAdmissionPolicy that refuses direct edits to the four
  kinds, for every identity but the GitOps reconciler.
- **Documentation**: Concepts, Guides, and a Reference generated from the code with a CI drift
  check, at <https://firestoned.github.io/notaio/>.

### Security

- The controller's RBAC is read on the policy kinds, status writes, `create`/`patch` on Events, its
  own Lease and ConfigMap writes in named namespaces; it cannot create, edit or delete a policy
  object.
- Its NetworkPolicy allows egress only to DNS, the API server and OpenBao, and ingress only to the
  health and metrics port from the monitoring namespace.

[0.3.2]: https://github.com/firestoned/notaio/releases/tag/v0.3.2
[0.3.1]: https://github.com/firestoned/notaio/releases/tag/v0.3.1
[0.3.0]: https://github.com/firestoned/notaio/releases/tag/v0.3.0
[0.2.0]: https://github.com/firestoned/notaio/releases/tag/v0.2.0
[0.1.0]: https://github.com/firestoned/notaio/releases/tag/v0.1.0
