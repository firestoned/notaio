# ADR-0009: Release images and their supply chain

- **Status:** Accepted
- **Date:** 2026-10-23
- **Deciders:** Erick Bourgeois
- **Related:** banlieue ADR-0006 (release and supply-chain pipeline), whose core this ports; ROADMAP
  M2 ("image build, signing and provenance; pin the deployment by digest")

## Context

Every notaio image so far was built by hand on a workstation and pushed with a personal
credential. `deploy/controller/20-deployment.yaml` names `ghcr.io/firestoned/notaio:0.0.1`, which
was never published. Nothing records how an image was built or proves it came from this
repository.

notaio sits on the trust path of every sandbox: it signs what mediatore and mediatore-guest
enforce. Its image is therefore as sensitive as its signing key. A consumer, a GitOps reconciler
or an auditor has to be able to check, without trusting the registry, that a given image digest
was built by this repository's CI from a given commit.

banlieue solved this in its ADR-0006 with multi-arch images, keyless Cosign signatures, SBOMs and
build provenance. Its Chainguard image variant and its Grype and VEX triage are the parts notaio
does not need yet.

## Decision

1. **One image, `ghcr.io/firestoned/notaio`,** from the existing distroless `Dockerfile`, for
   `linux/amd64` and `linux/arm64`. Each architecture's binary is built on a native runner and
   copied in; nothing is compiled inside the image build.
2. **When:** every pull request builds both binaries and the multi-arch image, and pushes nothing,
   so a fork's pull request needs no write access. A push to `main` publishes `sha-<short sha>`,
   `main-<date>` and `latest`. A published GitHub Release publishes `v<X.Y.Z>` and `v<X.Y>`, built
   with the workspace version set to the tag's.
3. **Every published image is signed and attested by digest:**
   - a keyless Cosign signature (GitHub's OIDC identity for this workflow, no key to hold);
   - BuildKit's SBOM and provenance attestations, attached to the image;
   - a GitHub build-provenance attestation (`actions/attest-build-provenance`);
   - a CycloneDX SBOM of the image, kept as a workflow artifact and, for a release, a release
     asset.

     *Amended 2026-10-27:* the SBOM of v0.1.0 listed only the distroless base's packages, because a
     plain release binary carries no record of its crates. The binary is now built with
     `cargo auditable` (Rust Secure Code working group, MIT OR Apache-2.0), which embeds its
     dependency tree, so the SBOM lists every Rust crate at the version built, and
     `make sbom-check` fails the image job when it does not.
4. **The digest pin is a release asset.** Each release attaches `notaio-<version>.yaml`: the CRDs,
   the controller manifests and the built-in profiles, with the image pinned to that release's
   digest. It is signed with keyless Cosign (`notaio-<version>.yaml.sigstore.json`), and its
   SHA-256 is in `checksums.sha256`. The repository's `deploy/` stays the readable source, naming
   `ghcr.io/firestoned/notaio:latest`; CI never rewrites tracked files.
   *Amended 2026-10-31:* the CRDs ship as their own asset, `notaio-<version>-crds.yaml`, signed and
   checksummed the same way, applied and waited for before `notaio-<version>.yaml`. In one file the
   built-in profiles failed a first install, because `kubectl` reads every kind before creating
   any. The per-namespace bundle Role left the install manifest for
   `deploy/policy-namespace/`: it named an example namespace every install would have had to
   create or edit. Both found by installing a release on a real cluster (roadmap 01).
5. **The admission policy is not in the install manifest.** `deploy/admission/gitops-only.yaml`
   names the site's GitOps identity, so each installation adapts and applies it itself.
6. **The logic lives in the Makefile** (`release-binary`, `release-manifest`, `set-version`) and
   is tested there, per the workflow rules. The workflow installs tools, and calls Make targets and
   the actions that can only run in CI (buildx, Cosign, attestation, release upload).

## Consequences

- An image digest from a release can be verified end to end without trusting the registry:
  `cosign verify` against this repository's workflow identity, and `gh attestation verify` for the
  build provenance.
- A GitOps installation pins the release manifest's digest, not a tag, so a re-pushed tag cannot
  change what runs.
- `latest` moves with `main`. It is for test clusters; production follows releases.
- Fork pull requests exercise the build and the image, but not the signing or the push, which run
  only for this repository's own events.
- Scanning the image (Grype) and VEX documents are left out, as is a second image variant. They are
  the follow-ups banlieue already runs, and fit after the first release.
- The first release is a decision of its own: cutting it exercises the release-only path (version
  stamping, the manifest and the asset upload) for the first time.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
