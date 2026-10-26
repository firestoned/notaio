# Changelog

Notable changes to notaio, by release, in the style of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/). Each release's images and install manifest are signed
and pinned by digest ([ADR-0009](https://github.com/firestoned/notaio/blob/main/docs/adr/0009-release-and-supply-chain.md)).

Add an entry under **Unreleased** in the pull request that makes a change a user would notice. At
a release, the maintainer renames Unreleased to the version and date.

## Unreleased

No release has been cut yet. Everything below is on `main`.

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
- **Health, metrics and Events**: `/healthz`, `/readyz` and `/metrics` on port 8080, two
  must-be-zero series, a fail-open guard before signing, and Events on transitions (ADR-0010).
- **Release pipeline**: multi-arch images built on native runners, keyless Cosign signatures,
  SBOMs, build provenance and a digest-pinned install manifest per release (ADR-0009).
- **GitOps-only admission**: a ValidatingAdmissionPolicy that refuses direct edits to the four
  kinds, for every identity but the GitOps reconciler.
- **Documentation**: Concepts, Guides, and a Reference generated from the code with a CI drift
  check, at <https://firestoned.github.io/notaio/>.

### Security

- The controller's RBAC is read on the policy kinds, status writes, `create`/`patch` on Events and
  ConfigMap writes in named namespaces; it cannot create, edit or delete a policy object.
- Its NetworkPolicy allows egress only to DNS, the API server and OpenBao, and ingress only to the
  health and metrics port from the monitoring namespace.
