# Changelog

Newest first. Every entry carries `**Author:**` (`update-changelog` skill).

## [2026-11-02 09:00] - ADR-0004 decision 6: OpenBao unseals itself

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0004-signing-key-custody.md`: decision 6 (the seal is a custody decision; the
  `static` seal for the test instance, `transit` or a KMS for production) and its accepted risk.
- `docs/architecture.md` known gaps: the test cluster's Flux identity (the first bullet was stale)
  and the seal key in the cluster.
- Roadmap 02 phase C done.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-01 22:00] - Roadmap 03: policies delivered as signed OCI artifacts

**Author:** Erick Bourgeois

### Changed
- `.github/community/03-signed-policy-artifacts.md` (new): two artifacts (`policies`, `ceilings`)
  each verified by its own Flux `OCIRepository`; `policies` signed keyless by CI after lint,
  `ceilings` by Security with a key it alone holds; what the artifact signature proves versus the
  bundle signature, which stays; phases A (kind, with the format and Rekor questions to confirm),
  B (the policy repository), C (the test cluster), D (an ADR, a guide, the threat model); the
  source-in-the-bundle idea left to its own ADR.
- `ROADMAPS.md`: M3 points to it, and a status board, one row per roadmap (00 to 03), which it
  lacked; roadmap 02 had been listed only in the M3 prose.

### Why
Roadmap 01's free-plan gap: anyone with push access changes what Flux applies. Signing the
ceilings by hand moves Security's approval from a field to a signature that Flux checks.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-01 21:00] - Roadmap 02 phase A: the test OpenBao unseals itself

**Author:** Erick Bourgeois

### Changed
- `dev/openbao/openbao.yaml`: `seal "static"`, its key in Secret `openbao-seal` mounted read-only
  into the OpenBao pod; the header states where the key sits and what that exposes.
- `dev/openbao/setup.sh`: creates the seal key before OpenBao starts; initialises with one
  recovery share; migrates an instance set up before (Shamir) with its old unseal key, which
  becomes the recovery key; fails, with OpenBao's log, when the key does not open it.
- `dev/openbao/check.sh`: the seal type is `static`, with a recovery seal.
- `crates/notaio/tests/e2e_openbao.rs`: after OpenBao is scaled back, the policy it could not sign
  is published with nobody unsealing anything.
- Docs: the OpenBao signer guide's "Unsealing after a restart" (the seals 2.7 carries, the
  migration, the two failure modes, rotation); troubleshooting's `503`; the developer page; the
  Makefile's comments.
- Roadmap 02: in progress, the static seal decided; phase A and its two documentation items
  ticked.

### Why
Roadmap 02. Proven on kind first: a Shamir instance from the old `dev/openbao` migrated by the new
`setup.sh` (both transit key versions unchanged, `check.sh` clean), a deleted pod unsealed again
in 3 s, a wrong key left sealed with `setup.sh` failing on it.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [x] Config change only
- [ ] Documentation only

## [2026-11-01 16:00] - Roadmap 02: OpenBao unseals itself

**Author:** Erick Bourgeois

### Changed
- `.github/community/02-unseal-the-signer.md` (new): the seal options OpenBao 2.7.1 carries
  (checked against the binary), the recommendation (the `static` seal, its key in a Secret), the
  alternatives (transit against an OpenBao outside the cluster; a cloud KMS) and the rejected
  sidecar; phases A (kind), B (the test cluster), C (docs, ADR-0004 amendment).
- `ROADMAPS.md` M3 and roadmap 01's risk point to it.

### Why
A restarted OpenBao stays sealed until a person unseals it; once mediatore enforces bundles, an
expired bundle refuses claims and mints.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-01 15:00] - A key id is trusted once; roadmap 00 phase 1 progress

**Author:** Erick Bourgeois

### Changed
- `crates/sandboxpolicy-bundle/src/verify.rs`: `TrustedKeys::insert` refuses a duplicate key id;
  `ids()`; `Debug` printing ids only. Test in `tests/bundle.rs`.
- Roadmap 00: status and phase 1 brought up to date with mediatore's roadmap 09 and ADR-0026
  (D3 decided for the shadow phase). CHANGELOG.md.

### Why
Building mediatore's key configuration showed the replacement: mediatore had to guard against it
itself, and every other consumer would have to as well.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-01 11:00] - Release prep: v0.3.3

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.3.3; `CHANGELOG.md` gains its `[0.3.3]` section (`next_floor`); the install
  guide's `V=` is 0.3.3.

## [2026-11-01 10:00] - The floor decision as a pure function

**Author:** Erick Bourgeois

### Changed
- `crates/sandboxpolicy-bundle/src/floor.rs`: `next_floor(held, payload)`, the ADR-0008 decisions
  2 and 3 alone; `accept` calls it, behaviour unchanged. A test drives every case through it.
- ADR-0008 amended (decision 5); the consumer guide shows the async pattern; CHANGELOG.md.

### Why
mediatore ADR-0026 keeps the floor in its async store, written in one transaction so replicas
cannot race it down. notaio's `VersionFloor` is synchronous; the pure function keeps the rule in
notaio and the I/O in the consumer.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-10-31 23:45] - Release prep: v0.3.2

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.3.2; `CHANGELOG.md` gains its `[0.3.2]` section (`make cluster-smoke`, the
  quieter expired watch, the gettext VEX); the install guide's `V=` is 0.3.2.

## [2026-10-31 23:30] - Expired watches logged quietly

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/src/watch.rs` (new, with `watch_tests.rs`): `deserves_warning` classifies a
  controller stream error. An expired watch (410 Gone), an `ObjectNotFound` and a
  `ReconcilerFailed` (already logged and counted by `error_policy`) are routine; anything else
  warns.
- `crates/notaio/src/main.rs`: both controllers log routine errors at `debug`, others at `warn`.
- Docs: CHANGELOG.md Unreleased; roadmap 01's phase A follow-up ticked.

### Why
Seen on the test cluster (roadmap 01, A7): a watch expiring logged a `WARN` though the watcher
re-lists by itself, which trains readers to ignore warnings.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-10-31 23:00] - Roadmap 01 phase D: the loop proven

**Author:** Erick Bourgeois

### Changed
- `.github/community/01-run-notaio-for-real.md`: phase D ticked with results (a tightening to a
  verified bundle and a moved floor; a widening failing `L014`, with the free plan unable to block
  the merge; a sealed signer reporting `SigningFailed` and recovering on unseal). The policy
  repository and the smoke-after-merge items ticked. One follow-up left: the expired-watch `WARN`.
- `ROADMAPS.md`: roadmap 01 stage 1 done.

### Why
The test runs and their commands are in the private policy repository's `TESTING.md`; this records
the results without naming the cluster.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-10-31 22:00] - Roadmap 01 phase C: the read-only smoke suite

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/tests/e2e_smoke.rs` (new) and `make cluster-smoke`: three read-only tests (the
  controller rolled out, ready and leading once with the must-be-zero series at 0 on every
  replica; every policy published with a bundle that verifies against the OpenBao key and matches
  its status and identity; a direct edit denied by GitOps-only admission, via a server-side dry
  run). 3 of 3 on the test cluster; on a cluster without admission the third fails, as it must.
- Docs: the developer page; roadmap 01 phase C; CHANGELOG.md. The commands and the run log live in
  the private policy repository's `TESTING.md`, since they name the cluster.

## [2026-10-31 21:00] - Roadmap 01 phase B done: policies arrive only through Flux

**Author:** Erick Bourgeois

### Changed
- On the test cluster: the Flux Operator v0.62.1 (installer checksum and image signature
  verified) and Flux v2.10.0 through a `FluxInstance`, syncing the private policy repository with
  the maintainer's own read-only deploy key. The `platform` Kustomization applied GitOps-only
  admission; `ns-sandboxes` the namespace, notaio's bundle Role, and the research agent.
- The research agent published bundle version 1, signed by the transit key, verified against
  OpenBao's public key; a direct edit as cluster-admin is denied by the admission policy.
- Roadmap 01: phase B ticked.

## [2026-10-31 20:00] - Roadmap 01 phase B: groups from the identity provider; review on the free plan

**Author:** Erick Bourgeois

### Changed
- On the test cluster: Dex (Helm, chart 0.25.2, revision 4 to 5) gained `loadAllGroups: true` and
  `teamNameField: slug` on its GitHub connector, so tokens carry `org:team` groups; it restricts no
  organisation, so everyone who could log in still can. Rolled out with no errors; discovery offers
  the `groups` scope. Rollback is `helm rollback dex 4`. The team `firestoned:sandbox-users` holds
  the maintainer.
- The policy repository is scaffolded (`notaio lint` clean, kustomize builds, the bundle Role still
  binds `notaio-system/notaio`), with Flux's read-only deploy key generated and GitHub's host keys
  checked against the ones GitHub publishes.
- Roadmap 01: the identity step done; the repository layout as built; a risk recorded: on GitHub's
  free plan a private repository has no branch protection, so review is a habit, not a gate.

## [2026-10-31 18:30] - Roadmap 01 phase A: v0.3.1 installed on the test cluster

**Author:** Erick Bourgeois

### Changed
- notaio v0.3.1 installed on the long-lived test cluster from its verified release files, following
  the install guide step by step: the namespace and OpenBao CA, the CRDs then the rest, the bundle
  Role in the first policy namespace. The NetworkPolicy's API server rule was narrowed to the API
  server's endpoints in a local copy only (real addresses stay out of the repository).
- Checked: two replicas on different nodes, one leader, the built-in profiles Valid, `/readyz`,
  the must-be-zero series at 0, RBAC (ConfigMaps in the policy namespace only, no policy edits). A
  real bundle published and verified through OpenBao, and refused against another key; the test
  objects removed.
- Roadmap 01: phase A ticked; a follow-up to log an expired watch quietly instead of as a warning.

## [2026-10-31 23:50] - VEX for CVE-2026-84243

**Author:** Erick Bourgeois

### Changed
- Verifying v0.3.1 found a new High in the base image, CVE-2026-84243 (glibc gettext reading
  `LANGUAGE`), unfixed upstream, so the gate passed but the finding count went from 6 to 7.
  `.vex/CVE-2026-84243.json` and its evidence: the binary imports none of the gettext family or
  `setlocale`. `make vex-evidence` holds with 20 statements.

## [2026-10-31 23:30] - Release prep: v0.3.1

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.3.1; `CHANGELOG.md` gains its `[0.3.1]` section: the install fixes, with an
  upgrade note for a 0.3.0 install that created the example Role.

## [2026-10-31 22:00] - Install fixes found on a real cluster: CRDs apart, no example namespace

**Author:** Erick Bourgeois

### Changed
- Roadmap 01 phase A began with a server-side dry-run of the v0.3.0 manifest against the test
  cluster. It found that the built-in profiles fail a first apply alongside their CRDs (kubectl
  maps every kind first; `make cluster-deploy` hid it by applying the CRDs separately), and that
  the manifest carried the `notaio-bundles` Role and RoleBinding for an example namespace.
- `make release-manifest` writes `notaio-<v>-crds.yaml` and `notaio-<v>.yaml`; `make
  release-manifest-sign` (new) Cosign-signs both, and the workflow calls it.
  `scripts/check_release_manifest.py` checks both: the CRDs alone in theirs, and no CRD,
  admission policy, or Role or RoleBinding outside `notaio-system` in the other. Checked by putting
  the old Role back: refused.
- `deploy/policy-namespace/notaio-bundles.yaml` (new): the bundle Role and RoleBinding with no
  namespace, out of `deploy/controller/10-rbac.yaml`; `kind-deploy` and `cluster-deploy` apply it
  into the e2e policy namespace.
- Docs: the install guide's download, apply and RBAC steps; the developer page's release
  verification; ADR-0009 amended; roadmap 01 (phase A's finding, and the Flux Operator, recorded
  with why it is used despite being AGPL-3.0 and single-vendor); CHANGELOG.md.

## [2026-10-31 18:00] - Roadmap 01: run notaio for real

**Author:** Erick Bourgeois

### Changed
- `.github/community/01-run-notaio-for-real.md` (new): stage 1 of running end to end. notaio
  installed from the signed release, Flux (raw manifests, pinned), a private policy repository with
  `notaio lint` in CI and CODEOWNERS on ceilings, GitOps-only admission naming Flux's
  kustomize-controller, a read-only `e2e_smoke` suite (the kind suites, which create policies as
  cluster-admin, stay on kind rather than exempting a namespace from admission), and three checks
  that prove the loop. No cluster, host or repository names, per the no-real-infrastructure rule.
- `ROADMAPS.md` M3 links it.
- On the test cluster: OpenBao unsealed (it had sealed on a pod restart); its transit key
  `notaio:v1` is intact, ed25519, not exportable, and matches the saved public key.

## [2026-10-31 15:00] - Docs audit: a glossary, the conformance vectors explained, stale code ranges

**Author:** Erick Bourgeois

### Changed
- `docs/src/glossary.md` (new, in the Reference nav and linked from the home page): the identifier
  families (`K` knobs by boundary, `R` steps, `L` lint rules by family, `I` input codes, the threat
  model's `T`, `S` and `O`, ADR, milestone, roadmap, key id, API and schema versions) and the terms
  used across the docs. Full code lists stay in the generated reference, linked, not copied.
- `docs/src/concepts/bundle.md`: what the conformance vectors are and why they exist, in prose; the
  consumer guide and generated list only showed how to run them.
- Stale ranges fixed: `docs/architecture.md` and the lint guide said `L001` to `L016` and `I001` to
  `I008` (now 18 and 9); both now point at the generated list instead of naming a range that goes
  stale. The security page gains rows for the endpoint rules (L007, L017, L018) and for the
  conformance vectors.

## [2026-10-31 12:00] - Roadmap 00: phase 0 complete with v0.3.0

**Author:** Erick Bourgeois

### Changed
- `.github/community/00-m3-integrate-the-consumers.md`: status "In progress"; the v0.3.0 release
  ticked, with what its verification found (CRDs at v1alpha2 only, conformance vectors identical
  to the generator's output).
- `ROADMAPS.md` M3: phase 0 done with v0.3.0, D1 and D2 decided and built, phase 1 next in
  mediatore, D3 to D6 open.

## [2026-10-31 23:00] - Release prep: v0.3.0

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.3.0; `CHANGELOG.md` gains its `[0.3.0]` section, opening with the upgrade
  steps for the breaking API and bundle changes.

## [2026-10-31 21:00] - The conformance kit

**Author:** Erick Bourgeois

### Changed
- `crates/sandboxpolicy-bundle/src/conformance.rs` (new): the `Suite`, `Vector` and `Expect`
  types, `suite()` over the checked-in `vectors.json`, the trusted keys, `error_name` (the stable
  name of a refusal), and `run`, which checks a vector with this crate's `accept`.
- `examples/conformance.rs` (new, needs the new `schema` feature): writes 14 vectors (4 accepted:
  first sight, same version, newer, new incarnation; 10 refused: downgrade, stale incarnation,
  forged signature, untrusted key under the trusted key's id, expired, not yet valid, digest
  mismatch, old schema, old payload type, malformed) with fixed seeds and clocks, and the payload's
  JSON Schema from the types. Deterministic: ed25519 signs deterministically.
- `tests/conformance.rs` (new): every vector gets its verdict from the real `accept`; the suite
  covers every refusal and has unique names; the suite and schema are for bundle v3. Checked by
  mutating one verdict: the test names the vector and both outcomes.
- The bundle crate's `schema` feature: JSON Schema derives on the payload types, off by default;
  `bundle-deps-check` still counts 36 crates.
- `Makefile`: `conformance`, `conformance-check` (in CI), and clippy on the bundle crate with the
  feature, so the example is linted.
- Docs: a generated reference page listing the vectors, a consumer guide section on running them,
  roadmap 00 (both phase 0 items done), CHANGELOG.md.

## [2026-10-31 16:00] - ADR-0014: how consumers get the bundle crates

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0014-consumer-distribution.md` (new), roadmap 00's D1: consumers pin a release tag as a
  git dependency and allow the source in `deny.toml`; nothing on crates.io before 1.0.
- `sandboxpolicy-types`: `schemars` optional, behind a `schema` feature that only
  `sandboxpolicy-api` enables; every `JsonSchema` derive is a `cfg_attr`. The generated CRDs are
  byte for byte unchanged; the types build with and without the feature.
- `make bundle-deps-check` (new, in the CI test job): the bundle crate's own tree may not contain
  schemars, kube, k8s-openapi, tokio, hyper, reqwest, rustls, ring or openssl. 36 crates today, down
  from those plus schemars. Checked both ways: turning the feature on for the bundle crate fails it.
- Docs: the consumer guide's dependency and `deny.toml` lines; CLAUDE.md's code rule names the
  check; roadmap 00 (D1 decided); CHANGELOG.md; the ADR index and nav.

## [2026-10-31 12:00] - ADR-0013: bundle v3, routes per audience, API v1alpha2

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0013-bundle-v3-routes-and-v1alpha2.md` (new), roadmap 00's D2: each audience carries
  `endpoints` and `rules` in mediatore's proxy shape (brokered, so any K4.1 step); direct egress
  becomes `{ endpoint, rules }` with its K4.1 limits kept; one endpoint, one way; `L017`, `L018`;
  ceilings approve endpoints per audience; bundle schema v3; the API moves to v1alpha2 with
  v1alpha1 retired, since serving both needs a conversion webhook (an inbound listener).
- Types: `EndpointRule`; `AudienceGrant.endpoints` and `.rules`; `EgressRule { endpoint, rules }`;
  `ExposedAudience.endpoints`; `CeilingAudience.endpoints`; `SCHEMA` and `PAYLOAD_TYPE` v3; every
  kind at v1alpha2. Endpoints and rules sorted in the canonical form.
- Lint: `L007` on direct egress in its new shape; `L017` and `L018` (new); `L014` per audience
  endpoint. `notaio lint`: `I009` refuses a retired API version by name instead of half-parsing
  it; `I001`'s text names ExposureCeiling too.
- Tests: five new evaluate tests (brokered endpoints at Hardened, every L017 case, L018 three
  ways, L007's new shape, L014 per endpoint and the exposure), the `I009` test, the RFC 8785
  fixture recomputed independently in Python (its bytes match), the schema test at v3, the rules
  table guard at 18. Every test passes.
- Examples, fixtures, generated CRDs, profiles and reference at v1alpha2; the hardened example
  gains an endpoint and a read-only rule, its ceiling the endpoint; `make policy-lint` clean.
- Docs: ADR-0002 amended; concepts (policies, ceilings, bundle), guides (write a policy, approve a
  ceiling, the install guide's upgrade warning), architecture, threat-model mapping, roadmap 00
  (D2 decided), ROADMAPS, CHANGELOG.md (breaking), the ADR index and nav.

## [2026-10-30 23:00] - Roadmap 00: the M3 plan; ROADMAPS.md

**Author:** Erick Bourgeois

### Changed
- `.github/community/00-m3-integrate-the-consumers.md` (new): the M3 plan, grounded in the mediatore
  and banlieue trees. Where things stand (policy is the broker's config; no policy field on any
  claim; the guest's only input is `/v1/me`; no signatures verified; per-audience `max_ttl`
  collapsed). Six decisions first: D1 crate distribution (git tag recommended; `schemars` behind a
  feature for the guest), D2 bundle schema v3 matching mediatore's per-audience endpoints and
  `(method, path-glob)` rules, D3 `AgentSandbox.spec.policyRef` and a group-eligibility admission
  policy, D4 the bundle over `/v1/me` with the keys in the VM image, D5 the gateway trusting the
  broker, D6 the policy version in tokens. Then four phases: notaio's contract and conformance
  vectors, mediatore in shadow, mediatore enforcing, the guest; then the gateway.
- Two stale items corrected: M3 said banlieue claims name a policy (the claim is mediatore's
  AgentSandbox now), and banlieue ADR-0109 defers a question to "mediatore ADR-0023", which is the
  VM image ADR; the gateway ADR it means is not in the mediatore tree.
- `ROADMAP.md` is now `ROADMAPS.md`, the status index of the global roadmap convention, with
  detail plans in `.github/community/`. Every reference updated; the documentation rule now
  describes the layout as in force. M5 no longer lists the LICENSE as open.

## [2026-10-30 21:00] - Release prep: v0.2.0

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.2.0 (`make set-version`); `CHANGELOG.md` gains its `[0.2.0]` section,
  adding the base image bump and the SPDX headers to what Unreleased already held.

## [2026-10-30 18:00] - SPDX licence headers and their check

**Author:** Erick Bourgeois

### Changed
- An SPDX header (`Copyright (c) 2026 Erick Bourgeois, notaio`,
  `SPDX-License-Identifier: Apache-2.0`, banlieue's format) on every tracked Rust, shell, Python,
  Makefile, Dockerfile and `.github` workflow file: 80 files, after any shebang.
- `scripts/check_license_headers.sh` (new) and `make license-check`: tracked files only, so local
  `node_modules` and virtualenvs never trip it, and the exact identifier, not any SPDX line. It
  rejects a file without a header and one under another licence. Generated output and Markdown
  are not checked, as in banlieue.
- `build.yaml`: a `license-check` job on every pull request, never gated on changed paths; the
  header's last "not here yet" note is gone. Make rather than banlieue's shared action, which
  cannot run locally and checks fewer kinds of file.
- `CONTRIBUTING.md` and `.claude/rules/documentation.md`: the header and the check.

## [2026-10-30 14:00] - Pin the transit key version (ADR-0004, amended)

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/src/openbao.rs`: `NOTAIO_OPENBAO_KEY_VERSION` (optional; 1 or more, anything else
  refused at startup), `sign_request` (pure) sends `key_version` when pinned, and `ensure_version`
  refuses a signature from any other version, so nothing is published under a key consumers were
  not given. Four new tests. The startup log names the pinned version.
- `e2e_openbao`: pin v99, which OpenBao lacks (SigningFailed: the pin reaches OpenBao), then the
  real version (Published, signed by it), then unpin; before the step that takes OpenBao down.
- ADR-0004 amended; the rotation guide stages rotations (pin, rotate, distribute, move the pin,
  retire) and its "known limitation" is gone; troubleshooting rows; a Deployment comment; the
  generated environment reference; ROADMAP M2; CHANGELOG.md.

## [2026-10-29 16:00] - ADR-0012: image scanning and VEX

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0012-image-scanning-and-vex.md` (new): Grype on every published image, SARIF to Code
  Scanning, a gate on fixable High and Critical; curated OpenVEX in `.vex/`, attested and attached
  to releases; a statement only on mechanical evidence, re-checked every pull request.
- Grype on the current image: 31 matches, all in Debian packages of the distroless base, none
  fixable, nothing in the 200 Rust crates. `.vex/` (new): 19 statements suppress 25 of them, each
  checked by hand against the advisory text and against the binary: zlib and libstdc++ are not
  linked; 13 libc advisories name entry points the binary does not import (banlieue's symbol map
  where it applied, each re-checked); the two ld.so setuid advisories need an RPATH it does not
  have. Six stay reported with no evidence to suppress them: CVE-2026-8674 (notaio does use the
  resolver), CVE-2026-89092 (nscd), CVE-2019-1010022 to 1010025. banlieue's prose statements for
  the old glibc CVEs were not copied; they called its binary statically linked, which it is not.
- `scripts/check_vex_evidence.py` (new) with eight tests (`make vex-evidence-test`): statements
  and evidence pair up; a newly linked library, imported symbol, RPATH, other architecture or a
  weaker Deployment fails. Holds for the real binary.
- `Makefile`: `grype-install` and `vexctl-install` (pinned version and SHA-256, into `.tools/bin`,
  idempotent), `vex-evidence`, `vex-evidence-test`, `vex-assemble`, `image-scan`, `image-gate`;
  `.vex/` is a build-relevant path. All run end to end on slate against the main image: 19
  statements, 6 SARIF results, gate passes.
- `build.yaml`: the evidence check after each architecture's build, the checker tests, a `scan`
  job (assemble, Cosign-attest `--type openvex`, scan, SARIF upload, VEX artifact, gate), and the
  VEX document among the release assets. The header's stale notes are gone. actionlint clean.
- Docs: architecture, security page, developer page (verifying the VEX attestation, adding a
  statement), threat-model mapping (the image row moves to implemented), ROADMAP, ADR index and
  nav, CHANGELOG.md.

## [2026-10-27 01:30] - v0.1.0 verification follow-ups: the SBOM and the verify docs

**Author:** Erick Bourgeois

### Changed
- Verifying v0.1.0 found its image SBOM listed 14 Debian packages and no Rust crates. `make
  release-binary` now builds with `cargo auditable` 0.7.7 (screened: MIT OR Apache-2.0, Rust Secure
  Code working group; a build tool, nothing new in the image), installed by `make cargo-auditable`
  in the Build workflow. Proven on slate: the same image's Syft SBOM goes from 0 to 200 crates.
- `make sbom-check` (new, in the image job): at least 50 Rust crates, including notaio,
  sandboxpolicy-bundle, sandboxpolicy-core, kube, rustls and tokio. It passes on the new SBOM and
  fails on v0.1.0's.
- Docs: Cosign 3 or later (Cosign 2 cannot see the new bundle format), and `DOCKER_CONFIG` for `gh
  attestation verify` past a stale ghcr login, in the install guide and the developer page.
  ADR-0009 amended for the SBOM. `CHANGELOG.md` Unreleased: Fixed.

## [2026-10-26 23:55] - Release prep: v0.1.0

**Author:** Erick Bourgeois

### Changed
- Found while preparing the first release: every internal path dependency pinned
  `version = "0.0.1"`, which Cargo reads as exactly 0.0.1. The release workflow stamps the tag's
  version into the workspace before building, so any release would have failed to resolve. The
  pins are gone (the workspace is `publish = false`, so a path dependency needs no version).
- `make set-version-test` now also stamps a scratch copy of the workspace with a new version and
  runs `cargo metadata` on it; it fails, with that diagnosis, if a pin comes back. Checked against
  the old pins: it fails.
- Workspace version 0.1.0 (`make set-version`), so `main` and the release agree; `CHANGELOG.md`
  gains its `[0.1.0]` section.

## [2026-10-26 23:30] - ADR-0011: leader election and replicas

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0011-leader-election.md` (new): one leader by a Lease `notaio` in the controller's
  namespace; our own loop, no new dependency; 15 s Lease, renew every 5 s, give up at 10 s; losing
  the Lease ends the process; the pod name is the identity; a namespaced Role only; two replicas,
  a PDB, preferred anti-affinity; why an overlap cannot loosen a sandbox (the consumer floor).
- `crates/notaio/src/leader.rs` (new): `decide` (pure: create, renew, take over, wait) with seven
  tests, including that the leader gives up before its Lease can expire; `Elector` (`acquire`,
  `hold`, `release`), every write by `resourceVersion`, a 409 meaning another replica won.
- `main.rs`: every replica runs its caches and the listener; only the Lease holder polls the
  controllers; losing the Lease returns an error, so the process exits; a clean shutdown releases.
  `NOTAIO_POD_NAME` is required; `NOTAIO_NAMESPACE` names the Lease's namespace.
  `reconcile.rs`: no write unless leading, checked again before signing. `metrics.rs`:
  `notaio_leader`.
- `deploy/controller/`: two replicas, anti-affinity, `NOTAIO_NAMESPACE`; the `notaio-leader` Role
  and RoleBinding; `25-poddisruptionbudget.yaml` (new). `check_release_manifest.py` expects them.
- e2e: `e2e_leader` (new, in the matrix and `E2E_SUITES`): one leader agreeing with its metrics,
  the leader's pod deleted, a new leader, the take-over counted, a policy published by it.
  `e2e_rbac`: its own Lease only, nowhere else, no delete or list.
- Docs: architecture, security page, install and troubleshooting guides, a `NotaioNoLeader`
  alert, the regenerated reference (environment, metrics), ROADMAP M2, CHANGELOG.md, CLAUDE.md,
  CONTRIBUTING.md, the ADR index and nav.

## [2026-10-26 21:00] - Governance, licence and release notes

**Author:** Erick Bourgeois

### Changed
- `LICENSE` (new): Apache-2.0, the canonical text from apache.org, verbatim. Confirms what
  `Cargo.toml` already said; the README's "assumption" caveat is gone.
- `GOVERNANCE.md` (new), adapted from banlieue's and foundation neutral: roles, lazy consensus,
  ADRs, the threat model as the source of truth for security posture, votes, maintainership,
  releases and support. `MAINTAINERS.md` (new).
- `CODE_OF_CONDUCT.md` (new): the Contributor Covenant 2.1, checked verbatim against the
  published text except the reporting contact, which is the maintainers.
- `CONTRIBUTING.md` (new): sign-off and signature, ADR first, the rules from `CLAUDE.md` a
  contributor needs before writing code, and the gate (`lint`, `test`, `manifests-check`,
  `docs-reference-check`).
- `CHANGELOG.md` (new): release notes in Keep a Changelog style, with an Unreleased section that
  summarises everything on `main`; no release has been cut.
- Docs: a Governance section (governance, maintainers, contributing, code of conduct) and the
  changelog under Reference, all as snippet stubs. README: Contributing and Licence sections.
- `.claude/rules/documentation.md`: where release notes and governance live.

## [2026-10-26 18:00] - Docs: the generated Reference section

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/src/reference.rs` (new) renders seven pages from the code: the CRD API (every
  field from the generated schemas), lint rules (`RULES`, `INPUT_RULES`), the knob register
  (`REGISTRY`, Lab-only knobs, grant lifetimes, numeric ceilings), the built-in profiles (the full
  knob by profile matrix), metrics (parsed from what `/metrics` renders for a sample snapshot, so
  labels are real), the command line (clap's help) and the environment variables. Seven tests,
  including one that fails when the code reads a `NOTAIO_*` variable the page does not list, or
  lists one the code no longer reads.
- `crates/notaio/src/bin/docgen.rs` (new); `make docs-reference` writes `docs/src/reference/`,
  `make manifests` now runs it too, and `make docs-reference-check` fails on drift. The Build
  workflow's test job runs the check.
- Doc comments on every CRD field that had none, so the reference and `kubectl explain` describe
  every field. Descriptions only: `make manifests` regenerated `deploy/crds/`; the bundle's
  canonical form is unchanged.
- The controller's environment names and defaults are constants in `notaio::cli`, and the OpenBao
  defaults in `notaio::openbao` are public, so the generated page reads the values the code uses.
  `notaio lint`'s help now names ExposureCeiling files too.
- Docs: the Reference nav; Concepts and Guides link to the generated pages, and the metrics and
  input-code tables in the guides are replaced by links, so each exists once.
- Rules: `CLAUDE.md` (run `docs-reference-check`; never hand edit `docs/src/reference`),
  `.claude/rules/documentation.md` and the `regen-manifests` skill.

## [2026-10-26 14:00] - Docs: the Guides section

**Author:** Erick Bourgeois

### Changed
- `docs/src/guides/` (new), in the site nav before Concepts: install a release (verify it, the
  three site-specific edits, the OpenBao CA, per-namespace bundle RBAC, the admission policy, a
  health check; management cluster only); set up the OpenBao signer; write a policy; grant a
  relaxation; approve an exposure ceiling; lint a policy repository (exit and input codes, the
  Actions job, a layout that keeps the ceiling review separate); consume a bundle (trusted keys,
  `accept` and every refusal); observe notaio (scraping, every metric, starter alerts, Events,
  probes); rotate the signing key; troubleshooting by `Ready` reason, signer error and pod symptom.
- Checked against the code and a running kind cluster: the Event field selector, lint's output for
  a missing ceiling (it names no exposure, so the guides use an empty ceiling to list it), the
  signer's error strings, and the example consumer's output.
- `docs/src/developer/index.md`: the lint and consumer sections now point to their guides.
- `docs/src/index.md`: links to Guides.
- `ROADMAP.md` M2: pinning the transit key version, found while writing the rotation guide. Today
  the controller signs with the latest version, so a rotation cannot be staged at consumers first.

## [2026-10-23 21:00] - Docs: the Concepts section

**Author:** Erick Bourgeois

### Changed
- `docs/src/concepts/` (new), in the site nav after Home: an overview; knobs and steps (the eight
  boundaries, approvers, the toxic-combination legs as `lint.rs` computes them, the numeric
  ceilings per step); one page each for SandboxProfile, SandboxPolicy (every field with the rule
  that checks it, every `Ready` reason), KnobGrant (its rules, phases, and how a relaxation ends
  without revocation) and ExposureCeiling; the bundle (contents, the seven checks
  `verify_bundle` makes, in order, and why the transport is untrusted); and the life of a change,
  with the eight gates between a pull request and enforcement. Claims about consumers are limited
  to what ADR-0002, 0003 and 0008 decide.
- `docs/src/index.md`: links to Concepts; ExposureCeiling in the diagram; "four CRDs". `README.md`:
  "four CRDs".
- `docs/architecture.md`: the observability bullet rewrapped.
- `Makefile` `docs-linkcheck`: skips the site's own absolute URLs, which are canonical tags and
  404 for any page not deployed yet, so the check reports real breakage only. Every relative and
  external link is still checked.

## [2026-10-23 18:00] - ADR-0010: health, metrics and Events

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0010-health-metrics-and-events.md` (new): one listener on 8080 serving `GET /healthz`,
  `/readyz` and `/metrics` only, reading no body; ingress from the monitoring namespace only;
  gauges computed from the caches at scrape time; a fail-open guard before signing; Events on
  transitions; `create`/`patch` on `events.k8s.io` Events. Amends the no-inbound and RBAC rules in
  `CLAUDE.md` and `docs/architecture.md` to match.
- `crates/notaio/src/http.rs`, `metrics.rs`, `guard.rs`, `events.rs` (new), each with tests: exact
  routes and refused methods over a real socket; the must-be-zero series present at 0 from the
  start; relaxations by phase, age and expiry; bundle age; each guard check failing on its own;
  Events only on transitions, notes cut to the API server's 1024 bytes.
- `crates/notaio/src/reconcile.rs`: the guard runs before signing (`Ready=False, FailOpenRefused`,
  nothing published); counters for published bundles, signing failures and reconcile errors;
  Events for policies, grants and profiles. `main.rs`: the listener's own caches of profiles,
  policies and grants (the controllers' stores wake only one readiness waiter, found on kind),
  readiness after their first list, the listener bound before the controllers start.
- `deploy/controller/`: the `http` port and both probes, `NOTAIO_POD_NAME`, the Events rule in the
  ClusterRole, and the NetworkPolicy's one ingress rule.
- e2e: `e2e_install` scrapes the deployed controller through a port-forward and checks both
  must-be-zero series; `e2e_rbac` checks Events are `create`/`patch` only; `e2e_bundle` and
  `e2e_openbao` wait for the `Published`, `NoApprovedCeiling` and `SigningFailed` Events.
- New dependency: `prometheus-client` 0.25.1 (Apache-2.0 OR MIT, the Prometheus project);
  `cargo deny check` passes. Dev only: kube's `ws` feature, for the port-forward.
- Docs: the architecture, the security page, the developer guide, the threat-model mapping (the
  relaxation metric moves to implemented), ROADMAP M2, the ADR index and the site nav.

## [2026-10-23 12:00] - ADR-0009: release images and their supply chain

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0009-release-and-supply-chain.md` (new): the core of banlieue's ADR-0006. One distroless
  image for amd64 and arm64 from native-runner builds; pull requests build and push nothing; `main`
  publishes `sha-`, `main-<date>` and `latest`; a release publishes `v<X.Y.Z>`, `v<X.Y>`; keyless
  Cosign by digest, BuildKit SBOM and provenance, `attest-build-provenance`, an image SBOM; each
  release attaches `notaio-<version>.yaml` pinned by digest, Cosign-signed, with checksums.
- `.github/workflows/build.yaml`: the `release` trigger and commit verification for it; jobs
  `extract-version`, `build`, `docker`, `attest`, `release-manifest`, `upload-release-assets`; the
  test job runs `release-manifest-test` and `set-version-test`. `actionlint` clean.
- `.github/actions/extract-version`, `.github/actions/prepare-docker-binaries` (new), ported.
- `Makefile`: `release-binary`, `release-manifest`, `release-manifest-test`, `set-version`,
  `set-version-test`. `scripts/check_release_manifest.py` (new): the image pinned by digest exactly
  once, every install kind present, the admission policy absent; checked against a wrong digest, a
  surviving tag and an included admission policy, and each is refused.
- `deploy/controller/20-deployment.yaml`: `ghcr.io/firestoned/notaio:latest` (was a placeholder
  tag that never existed); a release pins it by digest in its manifest. `imagePullPolicy:
  IfNotPresent` is explicit, because `latest` defaults to Always, and Always made every kind e2e
  suite try to pull its side-loaded `local-dev` image from ghcr and time out.
- Docs: the developer guide (verifying a release), the security page, the ADR on the site,
  `ROADMAP.md` (M2), `docs/threat-model-mapping.md` (the image's integrity implemented; scanning
  and VEX not yet).

### Why
ROADMAP M2: every image so far was built by hand. notaio signs what the sandboxes enforce, so its
image has to be checkable without trusting the registry.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [x] Config or examples change only
- [ ] Documentation only

## [2026-10-23 00:00] - OpenBao is the default signer; the development seed is opt-in

**Author:** Erick Bourgeois

### Changed
- `deploy/controller/20-deployment.yaml`: signs with OpenBao transit (address, CA from ConfigMap
  `openbao-ca`, projected token, audience `openbao`). The development seed's env, Secret mount and
  creation are gone. `30-networkpolicy.yaml`: one egress rule, to OpenBao on 8200.
- `deploy/signer/development-patch.yaml` (new) replaces `openbao-patch.yaml` and
  `openbao-networkpolicy.yaml`: the seed is now the opt-in, for local experiments.
- `dev/openbao/use-signer.sh`: `ca` (copy OpenBao's CA), `openbao` (the manifest as written),
  `development` (the opt-in patch and its Secret).
- `Makefile`: `kind-deploy` depends on `kind-openbao` and copies the CA; every suite gets
  `NOTAIO_E2E_OPENBAO_PUBLIC_KEY` (`E2E_ENV`); `kind-e2e-openbao` no longer switches signers;
  `cluster-deploy` copies the CA instead of creating a seed.
- `crates/notaio/tests`: `common::openbao_key` and `common::trusted_keys` (OpenBao's public key);
  `e2e_bundle` verifies against it, and its wrong-key check forges OpenBao's key id.
- Docs: ADR-0004 (implemented and the default), architecture, the security page, `SECURITY.md`,
  the developer guide, the e2e workflow's header, `ROADMAP.md` (M2), `docs/threat-model-mapping.md`
  (T8.2 implemented for the default deployment; the "not covered" row is gone).

### Why
ROADMAP M2 and threat model T8.2: the deployment as shipped no longer holds a key that whoever
administers the cluster can read. Every e2e suite now tests that configuration, not a development
one.

### Impact
- [x] Breaking change (a deployment needs OpenBao and ConfigMap `openbao-ca`; the seed is gone)
- [x] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-22 12:00] - Implement ADR-0004: the OpenBao transit signer

**Author:** Erick Bourgeois

### Changed
- `crates/sandboxpolicy-bundle/src/dsse.rs`: `signing_input` and `envelope` split signing in two, so
  a signer that works out of band (OpenBao) produces exactly what `sign_bundle` does; `sign_bundle`
  is built on them.
- `crates/notaio/src/openbao.rs` (new, in the library): `choose_signer` (OpenBao, development or
  none; both configured is refused; `https://` and a CA required), `TransitSigner` (TLS 1.3 against
  OpenBao's CA only, Kubernetes auth with the projected token re-read on every login, cached client
  token refreshed at four fifths of its lease, `transit/sign`, one fresh login on a 403, retries
  with backoff on transport errors and 5xx, a timeout per request), `parse_signature` (key id
  `<key>:v<N>`), `parse_login`. No new crate in the lockfile: the HTTP and TLS crates were already
  there through kube.
- `crates/notaio/src/signer.rs`: `BundleSigner` (development or OpenBao) with one async `sign`.
- `crates/notaio/src/reconcile.rs`: a signing failure publishes nothing, sets
  `Ready=False, SigningFailed` with the reason, and keeps the previous bundle's version and digest.
- `deploy/signer/openbao-patch.yaml`, `deploy/signer/openbao-networkpolicy.yaml` (new): run the
  controller with the OpenBao signer. `dev/openbao/use-signer.sh` (new) switches either way.
- `crates/notaio/tests/e2e_openbao.rs` (new): no development seed; a bundle signed by `notaio:v<N>`
  that verifies against OpenBao's public key; with OpenBao gone, `SigningFailed` and no bundle.
- `Makefile`: `kind-cert-manager` (v1.21.2, checksum, webhook readiness), `kind-openbao`,
  `kind-e2e-openbao`, `cluster-deploy`, `cluster-undeploy`, `openbao-e2e`; `openbao` in
  `E2E_SUITES`. `.github/workflows/e2e.yaml`: the OpenBao signer job; `dev/**` triggers the e2e.
- Unit tests: signer choice (six cases), signature and login parsing, split signing. 104 pass.
- Docs: ADR-0004 implemented, architecture, the security page, the developer guide, `ROADMAP.md`
  (M2), `docs/threat-model-mapping.md` (T8.2 implemented with `deploy/signer/`; the default
  deployment's gap stays open; the hard floor covers signing failures).

### Why
ROADMAP M2's first item and threat model T8.2: the signing key must not be held by whoever
administers the cluster.

### Impact
- [ ] Breaking change (the default deployment is unchanged)
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-22 00:00] - tokio 1.54.0 (yanked) to 1.54.1; cargo-deny denies yanked crates

**Author:** Erick Bourgeois

### Changed
- `Cargo.lock`: `tokio` 1.54.0 to 1.54.1, and nothing else.
- `deny.toml`: `yanked = "deny"`, so `make deny` fails on a yanked crate as cargo-audit does.
  Checked both ways: against the old lockfile it fails on tokio, with the fix it passes.

### Why
tokio 1.54.0 was yanked from crates.io on 2026-10-19, and cargo-audit (`--deny warnings`) failed the
Build's security scan on `main`. cargo-deny does not treat a yanked crate as an error, which is
why its job passed in the same run. Checked locally with cargo-audit, cargo-deny, clippy and the
93 tests.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [x] Config or examples change only
- [ ] Documentation only

## [2026-10-21 08:00] - Semgrep: exclude the `args` audit note; document the docker cooldown

**Author:** Erick Bourgeois

### Changed
- `.github/workflows/sast.yaml`: `--exclude-rule=rust.lang.security.args.args`. The rule flags every
  `std::env::args()` because `argv[0]` is untrusted; nothing here reads `argv[0]` (`crdgen` and the
  `verify_and_record` example skip it; `notaio` uses clap). The comment for the existing
  `unsafe-usage` exclusion now says why it holds here: `unsafe_code = "forbid"`.
- `.github/dependabot.yml`: the docker entry's 2-day cooldown is named as the one deliberate
  exception to Semgrep's 7-day `dependabot-missing-cooldown`; its alert is dismissed as won't fix.

### Why
Code scanning alerts #13 and #14 (`args`, INFO, audit) and #11 (cooldown). Checked with the
workflow's pinned Semgrep image and flags: one finding left, the deliberate cooldown.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [x] Config or examples change only
- [ ] Documentation only

## [2026-10-21 06:00] - A test OpenBao for the transit signer (ADR-0004)

**Author:** Erick Bourgeois

### Changed
- `dev/openbao/openbao.yaml` (new): OpenBao 2.7.1 with PostgreSQL 18 as its storage backend
  (persistent volume), TLS 1.3 from a namespaced cert-manager CA (`sslmode=verify-full` to
  PostgreSQL), the audit device declared in config, Kubernetes auth via `system:auth-delegator`,
  NetworkPolicies (only `notaio-system` reaches OpenBao; only OpenBao reaches PostgreSQL).
  Images pinned by digest, restricted Pod Security.
- `dev/openbao/setup.sh` (new): idempotent; random PostgreSQL password in a Secret, init and
  unseal (state outside the repo, mode 600), a non-exportable ed25519 transit key `notaio`,
  policy `notaio-sign` (sign only), role `notaio` for `notaio-system/notaio`, prints the public key.
- `dev/openbao/check.sh` (new), `make openbao-dev-check`: the ADR-0004 properties, checked from
  outside as the signer will use OpenBao. Exits non-zero on any surprise; cleans up after itself.
- `Makefile`: `openbao-dev` and `openbao-dev-check`, with no defaults for the cluster or the state
  directory.
- `.dockerignore`: `dev/`. Developer guide: a section on it.

### Why
ADR-0004's signer needs an OpenBao to develop and test against. Stood up on the k0s test cluster
and verified: data in PostgreSQL and the key unchanged across a restart; `notaio-system/notaio`
can sign and cannot export, rotate, read the key config, use another key, encrypt or list
policies; another ServiceAccount cannot log in; another namespace cannot reach OpenBao; the audit
log records each signature request. Then torn down and rebuilt from nothing with
`make openbao-dev` (25 seconds), `make openbao-dev-check` passing, and the restart cycle (sealed,
check fails, `make openbao-dev` unseals, same key) confirmed. Two early login failures from brand
new pods could not be reproduced in four later fresh runs; the check retries login, as the signer
will have to.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [x] Config or examples change only
- [ ] Documentation only

## [2026-10-21 00:00] - ADR-0008: the consumer's persisted version floor (M1 complete)

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0008-consumer-version-floor.md`: the floor is kept per namespace and name with the
  incarnation's `uid`; same `uid` never goes down; a different `uid` is accepted only if issued
  later, so a recreated policy works and an old-incarnation replay does not; verify, persist,
  then accept.
- `crates/sandboxpolicy-bundle/src/floor.rs` (new): `VersionFloor`, `MemoryFloor`, `FileFloor`
  (one JSON file, temporary file plus sync plus rename, directory synced; a damaged file is an
  error, never an empty floor), `accept`. `BundleError::StaleIncarnation`, `BundleError::Floor`.
  No new dependency, no Kubernetes dependency.
- `crates/sandboxpolicy-bundle/examples/verify_and_record.rs` (new): the consumer loop.
- `crates/sandboxpolicy-bundle/tests/floor.rs` (new): first accept, downgrade, re-delivery, raise,
  new incarnation, stale replay, failed verification leaves the floor alone, restart, missing and
  corrupt files, unwritable floor refuses, atomic write. `e2e_bundle` now runs `accept` with a
  `FileFloor` across a consumer restart. 93 pass.
- Docs: `docs/architecture.md` (contracts table, "who trusts what"), the developer guide (consuming
  a bundle), the ADR on the site, `ROADMAP.md` (M1 complete), `docs/threat-model-mapping.md` (pass
  stamped to ADR-0008).

### Why
ROADMAP M1's last item and threat model T5.4: a consumer that forgets its floor on restart can be
downgraded to an older, looser bundle that is still signed and still valid.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-20 18:00] - ADR-0007: ExposureCeiling, the approved maximum exposure

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0007-exposure-ceiling.md`: a fourth kind, one per policy by name; exposure must be
  covered (audiences, access, token lifetime, egress, tools, lease; grants count); no ceiling, no
  bundle; a `Security` approval required; enforced by `notaio lint` and by the controller; codes
  L014 to L016 and I008.
- `crates/sandboxpolicy-api/src/ceiling.rs` (new): `ExposureCeiling`, `ExposureCeilingSpec`,
  `CeilingAudience`.
- `crates/sandboxpolicy-core`: `lint_exposure`; `EvalInput.exposure_ceiling`; `evaluate` fails
  closed (`NoApprovedCeiling`, `ExposureAboveCeiling`, `CeilingNotApproved`) and still reports
  `maxExposure`; `RULES` gains L014 to L016.
- `crates/notaio`: the controller fetches the ceiling by the policy's name and watches ceilings;
  `notaio lint` reads ceilings, I008 for one without a policy; `crdgen` emits the fourth CRD.
- `deploy/`: the new CRD (regenerated), RBAC read on `exposureceilings`, the GitOps-only admission
  policy now covers them.
- `examples/exposureceiling-agent-default.yaml` (new), covering the example policy exactly.
- Tests: core (L014 audience, token, lease, a widening grant; L015; L016), `notaio lint` (L014,
  L015, I008), fixtures updated; e2e: a ceiling before every published policy, a new "no ceiling,
  no bundle" test, ceilings in the admission and RBAC suites. 81 pass.
- Docs: architecture (the four kinds, evaluation step 7), README, the security page, the developer
  guide, the ADR on the site, `ROADMAP.md` (M1), `docs/threat-model-mapping.md` (pass stamped to
  ADR-0007; the approved-ceiling validation moves from "Not covered" to "Implemented").

### Why
ROADMAP M1 and the threat model's "maximum exposure compared with its approved ceiling": widening
a policy now needs a second, separate approval, and a grant cannot exceed what Security approved.

### Impact
- [x] Breaking change (a policy without an ExposureCeiling publishes no new bundle)
- [x] Requires cluster rollout (apply the new CRD and RBAC, record ceilings before the controller)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-20 12:00] - ADR-0006: `notaio lint`, and `notaio controller`

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0006-notaio-lint-cli.md`: one binary with subcommands; profiles from the files then
  the built-ins; the clock is now, `--now` to pin it; text and SARIF; exit 0, 1, 2; input codes
  I001 to I007.
- `crates/notaio/src/lib.rs`, `cli.rs`, `lint.rs` (new): the CLI definition and the lint engine,
  as a library so they are testable. `lint.rs` reuses `lint_knobs` and `evaluate`.
- `crates/notaio/src/main.rs`: dispatches `controller` and `lint`; the bare `3600` is now
  `DEFAULT_BUNDLE_LIFETIME_SECONDS`.
- `crates/sandboxpolicy-core`: `GrantResult.findings` keeps the structured findings behind a
  rejection; `lint::RULES` is the rule table as data, tested against the documented table.
- `deploy/controller/20-deployment.yaml`: `args: ["controller"]`, parsed by a test.
- `crates/notaio/Cargo.toml`: `clap` 4.6 (MIT OR Apache-2.0), passes `make deny`.
- Tests: `tests/lint.rs` (engine, one test per input code), `tests/cli.rs` (the real binary: exit
  codes, SARIF, usage, the deployment's arguments), two in core. 73 pass.
- `Makefile`: `policy-lint` on `examples/` and `deploy/profiles/`, clock pinned because the
  examples carry fixed dates; run in the Build workflow.
- Docs: `docs/architecture.md`, `README.md`, the developer guide (with a policy-repository
  workflow that uploads SARIF), the ADR on the site, `ROADMAP.md` (M1),
  `docs/threat-model-mapping.md` (pass stamped to ADR-0006).

### Why
ROADMAP M1 and the threat model's "toxic-combination lint on every policy pull request": a policy
the controller would reject now fails review instead of reaching the cluster.

### Impact
- [x] Breaking change (the controller is started with `notaio controller`)
- [x] Requires cluster rollout (the deployment passes `args: ["controller"]`)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-20 00:00] - Implement ADR-0005: RFC 8785 canonical form, schema v2

**Author:** Erick Bourgeois

### Changed
- `crates/sandboxpolicy-bundle/src/jcs.rs` (new), `jcs_tests.rs` (new): RFC 8785 for integer-only
  JSON. Members sorted by UTF-16 code units, minimal escaping, integers within plus or minus
  2^53 - 1, floats refused. No new dependency (`serde_jcs` considered; the bound had to be enforced
  either way, and this crate runs as root in every guest).
- `crates/sandboxpolicy-bundle/src/{content,verify,error,lib}.rs`: `canonical_bytes`, `digest` and
  `BundlePayload::new` return `Result`; `BundleError::NonCanonical`; verification refuses content
  without a canonical form.
- `crates/sandboxpolicy-bundle/src/{content,dsse}.rs`: `SCHEMA` `.../bundle/v2`, payload type
  `...bundle.v2+json`.
- `crates/sandboxpolicy-core/src/evaluate.rs`: such content fails closed, no bundle,
  `Ready=False, NotCanonical`.
- Tests: RFC 8785 sorting and escaping vectors, integer bound and float refusal, a golden digest for
  the fixture computed independently in Python, schema v2, fail closed in `verify_bundle` and
  `evaluate`. 52 pass.
- `docs/adr/0002-bundle-contract.md`: amended; schema v2 frozen, with the golden digest test as the
  guard. `docs/adr/0005-canonical-form-rfc-8785.md`: implemented.
- `docs/threat-model-mapping.md`: first full pass under `rules/threat-modeling.md`, stamped
  2026-10-20 against ADR-0001 to ADR-0005. T5.4, the signed-bundle hard floor, T8.2 (admission,
  key custody with its accepted risk) and distribution updated. No change to `docs/air/`.
- `docs/architecture.md`, `ROADMAP.md` (M1).

### Why
ADR-0005, the last change to the content's byte form before mediatore integrates (M3). Any
consumer can now recompute the digest with any RFC 8785 library. The threat model pass closes the
ADD cycle for ADR-0003 to ADR-0005.

### Impact
- [x] Breaking change (bundle schema v2: every content digest changes; no consumer has accepted v1)
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-16 00:00] - ADR-0003, ADR-0004, ADR-0005: ADR-0002's open decisions

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0003-bundle-distribution.md`: the ConfigMap stays; mediatore reads it through the API
  and relays the envelope unchanged to guests over its mutual-TLS stream; no consumer trusts the
  transport.
- `docs/adr/0004-signing-key-custody.md`: the production key is a non-exportable OpenBao transit
  key; notaio may only request signatures, with Kubernetes auth bound to its ServiceAccount.
  Accepted risk recorded: a cluster admin acting as that ServiceAccount can request signatures
  (audited, key never leaves OpenBao), revisit for a malicious-admin threat model.
- `docs/adr/0005-canonical-form-rfc-8785.md`: content canonicalised with RFC 8785;
  `maxEgressBytes` bounded to 2^53 - 1; `SCHEMA` bumps to v2, which is the schema that freezes.
- `docs/adr/0002-bundle-contract.md`: amended, open decisions 1, 2 and 4 point at the new ADRs.
- `docs/architecture.md`, `docs/src/security/index.md`, `SECURITY.md`, `ROADMAP.md` (M1, M2), the
  docs site (stubs, ADR index, nav), and the rustdoc on `Signer` and `notaio::signer::from_env`.

### Why
ROADMAP M1: decide distribution, key custody and canonicalisation before the bundle schema is
frozen, because mediatore (M3) cannot integrate against a contract that may still change. ADD step
1: decisions recorded before code. The CALM step is deferred (no model yet), noted in each ADR.

### Impact
- [ ] Breaking change (ADR-0005's schema v2 will be, when implemented)
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [x] Documentation only

## [2026-10-15 00:00] - crdgen: Kubernetes formats for unsigned integers

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/src/bin/crdgen.rs`: CRDs are emitted with schemars' `uint8`/`uint16` formats
  rewritten to `int32` and `uint32`/`uint64` to `int64` (`FORMAT_REWRITES`).
- `crates/notaio/tests/manifests.rs`: `crds_use_only_formats_kubernetes_recognises` fails on any
  schema format in `deploy/crds/` the API server does not recognise.
- `deploy/crds/sandbox.firestoned.io.yaml`: regenerated. Ten `format` lines change; nothing else.
- `ROADMAP.md`: the M1 follow-up marked fixed.

### Why
Kubernetes 1.36 warns `unrecognized format` on every apply of the old CRDs (found on the k0s test
cluster). The widths are kept, and schemars' `minimum`/`maximum` are untouched, so validation does
not change. Verified with a server-side dry run on 1.36: the old CRDs warn, the new ones do not.

### Impact
- [ ] Breaking change
- [x] Requires cluster rollout (re-apply `deploy/crds/` to drop the warning; schema behaviour unchanged)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-14 01:00] - First run on a k0s test cluster (ROADMAP M1)

**Author:** Erick Bourgeois

### Changed
- `ROADMAP.md`: M1's first item marked done, with the result and two follow-ups (uint formats in
  the CRDs, a WARN on reconciling a deleted policy).
- `docs/architecture.md`: "Known gaps in v0" now reflects the k0s run.
- `deploy/admission/gitops-only.yaml`: header records where the policy has been tested.

### Why
CLAUDE.md names this as the first useful piece of work: run the controller on a k0s test cluster
and report what breaks before changing anything. The image (`ghcr.io/firestoned/notaio:dev`, pinned
by digest) was built from `main` at 7881316. All four e2e suites passed against it, NetworkPolicy
enforcement was proven with a labelled and an unlabelled listener, and everything was removed from
the cluster afterwards.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [x] Documentation only

## [2026-10-14 00:00] - Replace serde_yaml_ng with serde-saphyr; rustls 0.23.46

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/Cargo.toml`: `serde_yaml_ng` replaced by `serde-saphyr` 2.0.0 (MIT OR
  Apache-2.0, passes `make deny`). Used by `crdgen` and two test files only.
- `crates/notaio/src/bin/crdgen.rs`, `crates/notaio/tests/manifests.rs`,
  `crates/notaio/tests/e2e_install.rs`: `serde_saphyr::to_string` and
  `serde_saphyr::from_multiple`.
- `deploy/crds/sandbox.firestoned.io.yaml`: regenerated. Formatting only (quote style, folded long
  descriptions); parses to the same data as before. `deploy/profiles/builtin.yaml` is byte-identical.
- `Cargo.lock`: minimal change. `serde-saphyr` and `granit-parser` in; `serde_yaml_ng`,
  `unsafe-libyaml`, `ryu` and a duplicate `hashbrown`/`indexmap` out; `rustls` 0.23.45 to 0.23.46.

### Why
RUSTSEC-2024-0449 marks `unsafe-libyaml` unmaintained (a c2rust translation of libyaml, archived,
bugs not fixed upstream), and cargo-deny and cargo-audit failed the Build on `main`. `crdgen` is the
source of truth for the checked-in manifests, so it should not depend on it. RUSTSEC-2026-0346 to
0349 (rustls, fixed in 0.23.46) were published at the same time and failed the same jobs.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout (the CRD schema is unchanged)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-03 00:00] - How the AgentSandbox components relate

**Author:** Erick Bourgeois

### Changed
- `docs/architecture.md`: new "Where notaio sits in AgentSandbox" section. A platform diagram, the
  components and each one's relationship to notaio, the contracts that cross component boundaries
  (who writes, who reads, where defined, state), sequence diagrams for the life of a change and the
  life of a sandbox, and who trusts what. Planned consumer behaviour is marked M3 throughout.
- `README.md`: linked banlieue, mediatore and mediatore-guest; an AgentSandbox section with the
  platform diagram; the name's meaning.
- `docs/src/index.md`: the same links, a short AgentSandbox paragraph and the platform diagram.
- `docs/mkdocs.yml`: no creation date from git-revision-date-localized, which failed strict builds
  at random on uncommitted pages.

### Why
The relationships between the AgentSandbox components were spread across ADRs in three
repositories. The core docs now explain them in one place.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [x] Documentation only

## [2026-10-02 00:00] - Badges, SECURITY.md and a MkDocs site, as in banlieue

**Author:** Erick Bourgeois

### Changed
- `README.md`: workflow badges (Build, E2E, Documentation, CodeQL, SAST), OpenSSF Scorecard, Rust,
  docs site, security policy, status, issues, last commit. The status text now says the controller
  runs end to end on kind.
- `SECURITY.md`: private vulnerability reporting, response times, notaio-specific scope.
- `docs/mkdocs.yml`, `docs/pyproject.toml`, `docs/poetry.lock`, `docs/README.md`, `docs/src/`:
  MkDocs Material site. Canonical docs (architecture, threat model mapping, framework mapping, ADRs,
  ROADMAP, built-in profiles) are pulled in with snippets, never copied. Strict build.
- `.github/workflows/docs.yaml`: builds on PRs, publishes to GitHub Pages on push to `main`.
  No CALM jobs and no release chain yet.
- `.github/requirements/poetry.{in,txt}`, `.github/tools/linkinator/`: hash-pinned Poetry and
  linkinator, from banlieue.
- `.github/dependabot.yml`: pip (`/docs`) and npm (linkinator) ecosystems.
- `Makefile`: `docs`, `docs-serve`, `docs-linkcheck`, `docs-clean`.
- `docs/architecture.md`: the first "Known gaps in v0" item now reflects the kind e2e.
- `ROADMAP.md`: M5 progress. `.claude/rules/documentation.md`: the site and its stub rule.

### Why
Same public face and supply-chain posture as banlieue.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [x] Documentation only

## [2026-09-30 00:00] - Rename to notaio; port CI, e2e and Claude rules from banlieue

**Author:** Erick Bourgeois

### Changed
- Whole tree: the project is renamed from sicario to notaio. Crate and binary `notaio`
  (`crates/notaio`), namespace `notaio-system`, env vars `NOTAIO_*`, SSA field manager and
  `app.kubernetes.io/managed-by` label `notaio`, image and repository `firestoned/notaio`.
- `.github/workflows/build.yaml`: replaces `ci.yaml`. Makefile driven, SHA pinned, per-job path
  gating via `make ci-code-changed`, signed-commit verification, cargo-audit and cargo-deny.
- `.github/workflows/e2e.yaml`: new. One kind cluster per suite (`install`, `bundle`, `rbac`,
  `admission`), reusable via `workflow_call`.
- `.github/workflows/{codeql,sast,scorecard,dependabot-auto-merge}.yaml`, `.github/dependabot.yml`,
  `.github/CODEOWNERS`, `.github/codeql/codeql-config.yml`: ported from banlieue. CodeQL also
  analyses Python (`scripts/check_air_mapping.py`).
- `crates/notaio/tests/e2e_*.rs`, `crates/notaio/tests/common/mod.rs`: the e2e suites.
- `Makefile`: `help`, `fmt-check`, `clippy`, `deny`, `ci-code-changed`, the `kind-*` and
  `kind-e2e-*` targets.
- `Dockerfile`, `.dockerignore`: distroless image, pinned by digest, for the e2e.
- `.claude/rules/`: architecture-driven-development, threat-modeling, testing, rust-style,
  documentation, github-workflows, no-real-infrastructure, adapted to notaio.
- `.claude/skills/`: cargo-quality, regen-manifests, tdd-workflow, update-changelog.
- `CLAUDE.md`: ADD section and an index of the rules and skills.

### Why
Bring notaio under the same ADD discipline, CI shape and supply-chain posture as banlieue, and
give ROADMAP M1 ("run the controller end to end") an executable, repeatable form.

### Impact
- [x] Breaking change (namespace, env vars, field manager and label renamed)
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only
