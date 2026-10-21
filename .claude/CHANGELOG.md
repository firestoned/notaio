# Changelog

Newest first. Every entry carries `**Author:**` (`update-changelog` skill).

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
