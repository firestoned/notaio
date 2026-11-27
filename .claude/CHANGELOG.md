# Changelog

Newest first. Every entry carries `**Author:**` (`update-changelog` skill).

## [2026-11-26 22:54] - The controller can emit bundle schema v4, behind NOTAIO_BUNDLE_SCHEMA (roadmap 05 phase 4)

**Author:** Erick Bourgeois

### Added
- `NOTAIO_BUNDLE_SCHEMA` (`crates/notaio/src/cli.rs`): `v3`, the default, or `v4`; anything else,
  an empty value included, stops the controller at startup. In the environment reference and as a
  comment in `deploy/controller/20-deployment.yaml` (ADR-0026 decision 5).
- `EvalInput.bundle_schema`. Under `V4`, `compile_content` signs the policy's groups sorted
  (`subjects`, `[]` for none), a composite's includes as its status records them, and `from` on each
  entry an include supplied, carried through the sort; under `V3` every v4 member is left out and
  the bundle is byte for byte what it was. `notaio lint` evaluates as v3 (it signs nothing).
- The fifth fail-open check, `subjects-mismatch` (`guard.rs`, ADR-0026 decision 7): a v4 bundle
  whose signed groups are not the policy object's own, sorted, is not signed.
  `notaio_fail_open_total` pre-registers it.
- `crates/notaio/tests/e2e_schema_v4.rs`, `make kind-e2e-schema-v4` (sets the variable for the run
  and always unsets it), `E2E_SUITES`, a CI matrix row.
- Tests: six in `tests/evaluate.rs` (v3 by default, groups sorted, `[]` signed as nobody, the
  version moving once each way on a switch and v3's digest coming back, a composite's includes and
  origins, a reordering keeping the v4 digest), the setting's in `cli_tests.rs`, three guard tests.

### Changed
- Docs: ADR-0026 (decisions 5 to 7 built), `docs/architecture.md`'s bundle row, the threat model
  mapping's guard row, roadmap 05 phase 4, `ROADMAPS.md` row 05.

### Why
Roadmap 05 phase 4: release N must be able to emit v4 when an operator chooses, and keep emitting
exactly v3 until then.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [x] Requires cluster rollout (CRDs, profiles or controller): a new controller; nothing changes
      until `NOTAIO_BUNDLE_SCHEMA=v4` is set
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-26 22:16] - The bundle crate reads schema v3 and v4 (roadmap 05 phases 2 and 3)

**Author:** Erick Bourgeois

### Added
- `crates/sandboxpolicy-bundle/src/members.rs`: the v4 members (`Subjects`, `IncludeRef`,
  `Resources`, `Sources`) and the bundle's entry types `BundleAudience`, `BundleEgress` and
  `BundleTool`: the shared type flattened with an optional `from`, dereferencing to it, so the CRD
  is unchanged and an entry without `from` serialises exactly as before (ADR-0026 decisions 1, 2).
- `SCHEMA_V3`, `SCHEMA_V4`, `PAYLOAD_TYPE_V3`, `PAYLOAD_TYPE_V4`, `SchemaVersion`;
  `BundlePayload::new_v3` (refuses content with a v4 member), `new_v4` (refuses content without
  `subjects`), `schema_version()`; `BundleContent::has_v4_members()`; `VerifyOptions::min_schema`.
  `SCHEMA`, `PAYLOAD_TYPE` and `BundlePayload::new` still mean v3.
- `tests/v4.rs` (14 tests, first): a v4 round trip, out-of-band signing under the v4 type, each
  refusal, the constructors, `min_schema`, and a v4 golden digest computed outside the crate.
- Conformance kit: seven `v4-` vectors appended (24 in all; the 17 v3 vectors byte for byte as
  before) and `bundle-v4.schema.json`, generated with `subjects` required and the schema pinned.
  `bundle-v3.schema.json` is frozen as checked in. `PAYLOAD_SCHEMA_V4_JSON`.

### Changed
- `verify_bundle` reads v3 and v4: the payload type and schema must pair, a v3 payload carrying a
  v4 member is `Schema`, a v4 payload without `subjects` is `Malformed`, a schema older than
  `min_schema` is `Schema`. The content digest is now taken over the content exactly as signed,
  and content that does not re-serialise to those bytes is `Malformed`: recomputed from the parsed
  types, a member written `null` read as absent and verified, where any other verifier would
  refuse it. Tightens; every bundle notaio has signed verifies as before (the v3 golden, the
  v0.5.2 fixture, the v3 vectors).
- `signing_input` derives the payload type from the payload's schema and `SigningInput` carries
  it; `envelope(&SigningInput, key_id, signature)` uses it, so the OpenBao path cannot envelope a
  v4 payload under the v3 type. `signing_input` refuses a schema this crate does not write.
- The generator forges its refusal vectors itself rather than through `sign_bundle`, which will
  no longer sign an unknown schema.
- `sandboxpolicy-core`: `compile_content` builds the entry types, with every v4 member empty (the
  controller still emits v3); `lint_definitions_against_ceiling` takes any iterator of grants.
- `crates/notaio/src/signer.rs`: the OpenBao signer passes the signing input to `envelope`.
- Docs: ADR-0026 amended (decisions 4 and 8) and partly implemented; ADR-0002's amendment note;
  `guides/consume-a-bundle.md` (both schemas, the two rules another verifier must follow,
  `min_schema`); the generated conformance reference; `docs/architecture.md`'s bundle row (it said
  "schema v2, frozen"); the T5.4 row of the threat model mapping; roadmap 05 phases 0 to 3;
  `ROADMAPS.md` row 05.

### Why
Roadmap 05: consumers must read v4 before notaio emits it (ADR-0026). Release N carries this.

### Impact
- [x] Breaking change (public API): `envelope` takes the `SigningInput`; `BundleContent` gains
      four members and its entries are the bundle's own types (`.into()` from the shared ones);
      `VerifyOptions` gains `min_schema`. mediatore's tests that put an `EgressRule` or
      `AudienceGrant` into a bundle's content need `.into()` when it takes this release.
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-26 16:11] - L034: a subject group a consumer could not match exactly (roadmap 05 phase 1)

**Author:** Erick Bourgeois

### Added
- `crates/sandboxpolicy-core/src/lint.rs`: `L034`, an error, on each `subjects.groups` entry that
  is empty, has leading or trailing whitespace, holds a pattern character (`*?[]{}`), or repeats
  an earlier entry (the finding names it). Matching stays exact: case and prefix variants are
  different groups. In `RULES` and the documented table; runs in `lint_policy`, so in the controller
  and `notaio lint` alike, on the effective spec (a composite's own groups).
- Tests: `lint_tests.rs` (each malformed shape at its index, a repeat naming the first, exact
  matching, an empty list staying `L010`), `tests/evaluate.rs` (a malformed group compiles no
  bundle, `InvalidPolicy`), `crates/notaio/tests/lint.rs` (a repeat is an error at pull request
  time, at `subjects.groups[1]`).

### Changed
- `docs/src/reference/lint-rules.md` regenerated; `RULES` has 33 codes.
- `docs/threat-model-mapping.md`: the eligibility row lists `L034` among the checks on
  `subjects.groups`; the row stays under "Not covered yet" until a consumer checks the signed list.
- ADR-0024 partly implemented (decision 3); roadmap 05 phase 1 done; `ROADMAPS.md` row 05.
- `crates/notaio/src/lint.rs`: `Diagnostic::code`'s doc no longer names a stale range of codes.

### Why
ADR-0024 decision 3: once the groups are signed and a consumer matches them exactly, a pattern, a
stray space or an empty entry is a grant nobody meant. It only adds findings, so it ships before
the schema.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [x] Requires cluster rollout (CRDs, profiles or controller): a new controller refuses a policy
      whose groups are malformed, which an older one published
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-26 15:20] - ADR-0026: bundle schema v4 and its rollout (roadmap 05 phase 0)

**Author:** Erick Bourgeois

### Added
- `docs/adr/0026-bundle-schema-v4-and-its-rollout.md`: the one v4 migration. The member table
  (`subjects` always, `includes` and `from` on a composite, places for `resources` and `sources`,
  optional members omitted and never `null`); `from` in bundle-crate wrappers, not the CRD's types;
  payload type and schema paired, and the payload type carried through out-of-band signing; one
  verifier for v3 and v4 for two releases; `NOTAIO_BUNDLE_SCHEMA` as the producer's switch; a fifth
  fail-open check, `subjects-mismatch`; the v4 conformance vectors; three releases with notes
  written for consumers. Accepted, not implemented. Docs-site stub, index row and nav entry.

### Changed
- `docs/adr/0024-subjects-in-the-signed-bundle.md`: accepted with ADR-0026; decision 6's vectors
  path corrected to `crates/sandboxpolicy-bundle/conformance/vectors.json`.
- `docs/adr/0002-bundle-contract.md`: amended by ADR-0026 (not implemented yet).
- `docs/adr/0016-source-in-the-bundle.md`, `docs/adr/0019-resource-sizes.md`,
  `docs/adr/0023-composed-policies.md`: their schema half is ADR-0026's.
- Roadmap 05 phase 0 (the notaio half) done; `ROADMAPS.md` row 05.

### Why
Four ADRs add bundle members and none owned the migration: how consumers read two schemas, how the
producer switches, when v3 ends. ADD's first step for roadmap 05.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [x] Documentation only

## [2026-11-26 14:52] - Roadmap 05: bundle schema v4, one migration, consumers first

**Author:** Erick Bourgeois

### Added
- `.github/community/05-bundle-schema-v4.md`: the one v4 migration that ADR-0016, ADR-0019,
  ADR-0023 decision 7 and ADR-0024 each call for. What a survey of notaio, mediatore and banlieue
  found (the exact schema check, the digest recomputed from parsed content, `envelope()` and
  `signing_input()` fixed to the v3 payload type on the OpenBao path, the CRD's types shared with
  the bundle, mediatore as the only consumer), eight decisions with recommendations (a migration
  ADR, the v4 member table, the dual verifier, `from` in the bundle crate, a producer switch,
  running claims at the cutover, a guard check on the signed groups, hand-written release notes),
  and the work over three releases.

### Changed
- `ROADMAPS.md`: row 05; under M4, the v4 and eligibility bullet, the K2.4 bullet pointing to
  roadmap 05, and roadmap 04 "in progress" rather than "planned".
- Roadmap 04, phase 4: built as part of roadmap 05.

### Why
Eligibility by group (K7.2) is enforced nowhere until the bundle carries the groups, and four
ADRs share one schema change that has to reach consumers before notaio emits it.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [x] Documentation only

## [2026-11-26 14:45] - ADR-0024 and ADR-0025 on the docs site; L030 to ADR-0019, L037 to ADR-0022

**Author:** Erick Bourgeois

### Changed
- `docs/src/adr/0024-subjects-in-the-signed-bundle.md`,
  `docs/src/adr/0025-a-ceiling-shared-by-several-policies.md`, `docs/src/adr/index.md`,
  `docs/mkdocs.yml`: the stubs, index rows and nav entries #117 left out.
- `docs/adr/0022-a-registry-audience-marked-by-role.md`: its lint code is `L037`. ADR-0019
  reserved `L030` first (amended 2026-11-20), and the code already said so.
- `crates/sandboxpolicy-core/src/lint.rs`: the module doc lists every code reserved by a proposed
  ADR (L030, L034 to L037) and the next free one (L038), so the next ADR does not collide.
- Roadmap 04: phase 0 done; mediatore rejected its ADR-0065 and withdrew roadmap 16 (#165).

### Why
Two proposed ADRs claimed `L030`; a stable code is a contract once it ships, so the collision is
fixed while neither is implemented.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [x] Documentation only

## [2026-11-26 11:43] - Poetry lock recompiled for Python 3.11 again

**Author:** Erick Bourgeois

### Fixed
- `.github/requirements/poetry.txt`: recompiled with the command in its header
  (`uv pip compile --python-version 3.11 ...`). Dependabot's poetry 2.3.4 bump (#113) had
  recompiled it without the 3.11 target and dropped eight packages Python 3.11 needs
  (`typing-extensions`, `importlib-metadata`, `zipp`, `cachecontrol`, `backports-tarfile`,
  `backports-zstd`, `lazy-importer`, `pbs-installer`), so the Documentation job failed installing
  Poetry on main. Verified with `pip install --require-hashes` in `python:3.11-slim`.

### Why
The same failure as the msgpack bump (#105). The docs job was red on #113, #114 and #115, and all
three merged: approving a held lock PR enables auto-merge, and the docs check is not required on
`main`.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [x] Config or examples change only (CI)
- [ ] Documentation only

## [2026-11-25 19:43] - kube 5.2.0

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/Cargo.toml`, `crates/sandboxpolicy-api/Cargo.toml`, `Cargo.lock`: kube,
  kube-client, kube-core, kube-derive and kube-runtime 5.1.0 to 5.2.0. `k8s-openapi` stays at
  0.29.0 and no other crate moves.

### Why
Keep the controller on the current kube-rs release. Same licence (Apache-2.0), same maintainers,
no new transitive dependency; `make deny` passes.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [x] Requires cluster rollout (CRDs, profiles or controller): a new controller image, no CRD
      change (`make manifests-check` passes, the generated CRDs are byte for byte the same)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-25 19:30] - Threat model pass for ADR-0023, composed policies (roadmap 04 phase 5)

**Author:** Erick Bourgeois

### Changed
- `docs/threat-model-mapping.md`: full pass, stamped 2026-11-25 against ADR-0001 to ADR-0023. A
  new "Implemented here" row for composition (L031 to L033, an include's KnobGrants never
  reaching a composite, `publish: false` never signed, a missing include publishing nothing, the
  watch); the O3, ceiling and status rows say what they mean for a composite; the T8.2 "silent
  edit" row notes that an include's owner changes every composite, bounded by each composite's
  ceiling; "Not covered yet" lists the unsigned origin of a grant until bundle schema v4.
- `docs/threat-model-mapping.md`, corrections found by checking banlieue and mediatore during the
  pass: "Profile eligibility by group (K7.2, T7.3)" claimed banlieue admission enforces it, and
  nothing does (`subjects.groups` is not in the bundle and no consumer checks it), so it moves to
  "Not covered yet" with the namespace-per-tenant mitigation and roadmap 00 item D3. The
  enforcement row now says mediatore reads audiences, egress and lease but no knobs, tools or
  budgets; that shadow, its default, means the ceiling bounds what is signed and not what runs;
  and that under `enforce` a notaio outage past the bundle lifetime fails closed.
- `docs/air/mapping.yaml` (`mi-12`) and the generated `docs/framework-mapping.md`: the same gap
  for subjects.
- `docs/adr/0023-composed-policies.md`: implemented but for decision 7's v4 fields.
- Roadmap 04 and `ROADMAPS.md`: the phase 5 threat model item done.
- The ADR is numbered 0023: main took ADR-0022 for the registry audience (#111) while this
  branch was open, and every reference on the branch moved with it.

### Why
ADD's last step: an ADR is not implemented until the pass is done. No knob, profile, RBAC rule,
admission rule, listener or bundle format changed, so `registry.rs` and `builtin.rs` stand as they
are. The corrections are coverage the mapping overstated before this ADR, not a change in notaio. ADR-0022 (registry audience) is proposed and has nothing implemented to map.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [x] Documentation only

## [2026-11-24 08:15] - Composed policies in the controller: the watch, includedBy, e2e (roadmap 04 phase 3)

**Author:** Erick Bourgeois

### Added
- `crates/notaio/src/include.rs` and `include_tests.rs`: `included_by` and `related`, the pure
  relations between composites and includes (ADR-0023).
- `crates/notaio/tests/e2e_compose.rs`, `make kind-e2e-compose`, `E2E_SUITES` and the
  `.github/workflows/e2e.yaml` matrix row: a composite of two `publish: false` policies published
  and verified; the includes publish nothing and report `includedBy`; reordering `includes` keeps
  the digest and version; an include widened past the composite's ceiling stops the re-sign with
  the last bundle kept; a deleted include reports `IncludeNotFound`.

### Changed
- `crates/notaio/src/main.rs`: a second watch on `SandboxPolicy` re-evaluates the policies that
  include the changed one, and the ones it includes now or last included. Deletions included.
- `crates/notaio/src/reconcile.rs`: lists the namespace's policies once per reconcile, from the
  API, to resolve includes and to write `status.includedBy`. Replaces the per-name reads.
- `crates/sandboxpolicy-api/src/policy.rs`: `status.includes` and `status.includedBy` always
  serialized: status is written as a JSON merge patch, so an omitted empty list would never clear.
- `deploy/crds/`, `docs/src/reference/`: regenerated.
- ADR-0023, `docs/architecture.md`, roadmap 04 and `ROADMAPS.md`: phase 3 marked done.

### Why
ADR-0023 decision 6: a change to an include must reach every composite promptly, not at the next
half-life refresh, so a widening past a composite's ceiling stops its re-signing at once.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [x] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-24 01:30] - Composed policies: spec.includes, L031 to L033 (roadmap 04 phases 1 and 2)

**Author:** Erick Bourgeois

### Added
- `crates/sandboxpolicy-api/src/policy.rs`: `spec.includes` (at most `MAX_INCLUDES`, four, a
  list-map keyed by `name`), `spec.publish` (default `true`), a CEL rule refusing `includes` with
  `publish: false`, and `status.includes` (`name`, `uid`, `contentDigest`) and
  `status.includedBy` (ADR-0023).
- `crates/sandboxpolicy-core/src/compose.rs` and `compose_tests.rs`: `compose`, the effective
  policy with the origin of every named entry, `content_digest` of what an include contributes,
  and L031 (two includes supply one key), L032 (nested includes, and the list's shape for
  `notaio lint`) and L033 (subjects wider than an include's). Findings on an included entry name
  the include.
- `crates/notaio/src/lint.rs`: includes resolved across the input files; `I012` for an include
  not in the inputs.
- `examples/sandboxpolicy-composite.yaml`, `examples/exposureceiling-build-agent.yaml`: a
  composite of two `publish: false` policies under its own ceiling.

### Changed
- `crates/sandboxpolicy-core/src/evaluate.rs`: `EvalInput.includes`. Lint, the ceiling checks
  (L014 to L016) and compilation run on the effective spec; `IncludeNotFound` (retried every
  minute) and `NotPublished` (Ready, no bundle, no ceiling needed) reasons.
- `crates/notaio/src/reconcile.rs`: fetches a composite's includes by name in its namespace. No
  watch on includes yet (phase 3): an include's change reaches a composite at its next refresh.
- `crates/sandboxpolicy-core/src/lint.rs`: L031 to L033 in `RULES` and the table; L030 stays
  reserved for ADR-0019.
- `deploy/crds/`, `docs/src/reference/`: regenerated (`make manifests`).
- ADR-0023, `docs/architecture.md`, roadmap 04 and `ROADMAPS.md`: phases 1 and 2 marked done.

### Why
ADR-0023: a sandbox that needs several policies' grants names one policy, and one ceiling
approved by Security bounds the combination (ADR-0007, objective O3). The threat model pass is
roadmap 04 phase 5, once the controller watch and the e2e suite land.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [x] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-24 00:11] - ADR-0023: a policy composed of others (roadmap 04 phase 0)

**Author:** Erick Bourgeois

### Added
- `docs/adr/0023-composed-policies.md`: `SandboxPolicy.spec.includes` (one to four policies in the
  same namespace, one level), named parts only, the composite's own entry replacing an include's,
  collisions refused (L031), no nested includes (L032), subjects no wider than every include's
  (L033), L014 to L016 on the effective content against the composite's own ceiling,
  `spec.publish: false` for a policy that exists only to be included, and in bundle v4
  `includes` and per-entry `from` as audit data only. notaio gains no
  `delete` on ConfigMaps: a policy that stops publishing has its old bundle expire (ADR-0018).
- `docs/src/adr/0023-composed-policies.md`, `docs/src/adr/index.md`, `docs/mkdocs.yml`: the site
  stub, index row and nav entry.

### Changed
- `docs/architecture.md`: a "Composed policies" section under Evaluation (the CALM step,
  deferred until the model is bootstrapped).
- `.github/community/04-composed-policies.md`, `ROADMAPS.md`: roadmap 04 in progress, phase 0's
  ADR item done, D6 and D7 decided, the ADR number (0022, not 0023), `I012` for a missing
  include, and phase 1's field in `sandboxpolicy-api` with `make manifests`.

### Why
A sandbox names one policy, and a workload needing several policies' grants had only a copied
policy with no ceiling. Composing in notaio keeps one ceiling approved by Security over the
combination and one signed bundle per sandbox (ADR-0007, T5.4, objective O3), where mediatore
ADR-0065's composition at the broker left the union unbounded.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [x] Documentation only

## [2026-11-23 04:30] - Release prep v0.8.0

**Author:** Erick Bourgeois

v0.8.0 is the fixes from the 2026-11-22 security review (#109): status written only by notaio
(ADR-0021), consumers refusing another policy's bundle and a lifetime over one day (ADR-0008
decision 7, ADR-0002 decision 5 as amended), the `FileFloor` temporary file, and `notaio lint`
refusing symlinks. v0.8.0-rc.1 ran on the test cluster first, admission included.

### Changed
- `Cargo.toml` and `Cargo.lock`: the workspace crates at 0.8.0. Minor, not patch: `floor::accept`
  and `floor::accept_audience` changed signature.
- `docs/src/guides/consume-a-bundle.md`: the dependency example pins `v0.8.0` (it said `v0.3.0`,
  older than the `accept` it documents).

### Added, in #109 without an entry
- `.github/workflows/docs.yaml` also runs on `.github/requirements/**`, so a change to the
  hash-locked Poetry runs the job that installs it.
- `.github/workflows/dependabot-auto-merge.yaml`: a Dependabot pull request for
  `/.github/requirements` is held for review, not auto-merged, with a comment saying why
  (`hold-reason`). Dependabot recompiles that lock without its Python 3.11 target (#105), and the
  e2e gate never installs it. An approval still lets it merge.

## [2026-11-22 22:30] - Docs CI: the Poetry lock targets Python 3.11 again

**Author:** Erick Bourgeois

### Fixed
- `.github/requirements/poetry.txt`: Dependabot's msgpack bump (#105) regenerated the lock without
  `--python-version 3.11`, dropping `importlib-metadata` and `zipp`, which `keyring` needs below
  Python 3.12, so the Documentation workflow's `--require-hashes` install failed on the next pull
  request that touched the docs. Regenerated with the command in the file's header, keeping the
  versions the lock held before (`importlib-metadata` 9.0.0, `zipp` 4.1.0) rather than a newer
  release that adds a dependency. Against the last working lock, only Dependabot's certifi and
  msgpack bumps differ. Checked with a hash-pinned install in `python:3.11-slim`.

## [2026-11-22 16:00] - Consumer verification and lint input hardening (ADR-0002, ADR-0008)

**Author:** Erick Bourgeois

The Low findings of the 2026-11-22 security review, in the same pull request as ADR-0021.

### Added
- `sandboxpolicy-bundle`: `VerifyOptions::max_lifetime`, defaulting to the new
  `MAX_BUNDLE_LIFETIME_SECONDS` (one day), and `BundleError::LifetimeTooLong`. A consumer refuses
  a bundle whose `notAfter` is more than that after its `issuedAt`, for policy and audience
  bundles alike (`check_window`). ADR-0002 decision 5, amended.
- `sandboxpolicy-bundle`: `floor::check_policy`, `floor::check_audience` and
  `BundleError::WrongSubject`. ADR-0008 decision 7 (new).
- Conformance vectors: `refused-wrong-policy`, `refused-wrong-audience` and
  `refused-lifetime-too-long` (both kinds), now 17 per suite; existing vectors are byte for byte
  unchanged. `docs/src/reference/conformance.md` regenerated.
- Tests: the lifetime cap (`tests/bundle.rs`, `tests/audience.rs`), the subject check
  (`tests/floor.rs`, `tests/audience.rs`), the file floor's temporary file (a planted symlink, a
  leftover from a crash, mode `0600`), and lint inputs with symlinks (`crates/notaio/tests/lint.rs`,
  red before the fix).

### Changed
- **Breaking (crate API):** `floor::accept` and `floor::accept_audience` take the namespace and
  name the consumer asked for, and refuse any other bundle before the floor is read. No consumer
  called them (mediatore uses `next_floor`). `conformance::run` and `run_audience` ask for the
  suite's own subject. The `verify_and_record` example takes `<namespace> <policy>`.
- `FileFloor` creates its temporary file exclusively (`create_new`, never through an existing file
  or symlink) with mode `0600`, unlinking a leftover from a crashed process with the same pid and
  trying once more. mediatore-guest will run it as root (ADR-0008 decision 5, amended).
- `crates/notaio/src/cli.rs`: `MAX_BUNDLE_LIFETIME_SECONDS` is the bundle crate's constant, so the
  controller's maximum and the consumers' cannot drift.
- `notaio lint` refuses a symlink found while walking a directory (an input error), instead of
  following it: a pull request could loop the walk or have it read outside the tree. A path named
  on the command line is still followed. ADR-0006 decision 3 notes it.
- `docs/src/guides/consume-a-bundle.md`, `docs/src/concepts/bundle.md`,
  `docs/src/developer/index.md`: the new `accept` signature, the two refusals, the floor file's
  permissions, the example's arguments, the vector count.
- `docs/threat-model-mapping.md`: full pass, stamp now also names ADR-0002, ADR-0006 and ADR-0008
  as amended 2026-11-22. T5.4, the lint row and consumer downgrade protection updated; no other
  row changed.
- `docs/src/adr/index.md`: removed a duplicated ADR-0021 row.

### Not changed, by decision
- The Deployment's `:latest` image: release installs use the digest-pinned manifest (ADR-0009).
- NetworkPolicy egress on 443 and 6443: narrowing it to the control plane is a per-site step the
  manifest header and the install guide already require.
- Watching bundle ConfigMaps to republish a deleted one at once: it would widen what the
  controller reads (T8.2) to shorten an outage already bounded by the refresh.

## [2026-11-22 12:00] - Status is written only by notaio (ADR-0021)

**Author:** Erick Bourgeois

### Added
- `docs/adr/0021-status-written-only-by-notaio.md`, with its docs-site stub, index row and nav
  entry. The GitOps-only admission rule named the main resources, which do not match their
  `/status` subresources, while notaio reads status back as input: the previous bundle version and
  digest, the refresh time, and an audience's standing in a collision. Any identity with RBAC to
  patch status could make notaio publish below what consumers hold, or have a platform audience's
  change refused. Found in a security review on 2026-11-22.
- `deploy/admission/gitops-only.yaml`: a second ValidatingAdmissionPolicy and binding,
  `sandbox-status-notaio-only`, matching `UPDATE` of the status of `SandboxProfile`,
  `SandboxPolicy`, `KnobGrant` and `SandboxAudience` and admitting only
  `system:serviceaccount:notaio-system:notaio`. Same file, so it is applied and removed with the
  spec rule.
- `crates/notaio/tests/e2e_admission.rs`: `status_writes_are_denied_even_for_cluster_admin`, and
  the GitOps test now waits for both rules, checks the controller still publishes, and checks
  cluster-admin is refused a status patch on the published policy.

### Changed
- `docs/threat-model-mapping.md`: full pass, stamped 2026-11-22 against ADR-0001 to ADR-0021. A
  new "Implemented here" row for status; the T8.2 and collision rows cite ADR-0021. The guest rows
  now say what the code does: mediatore verifies bundles and keeps the floor with `next_floor`,
  refusing a bundle whose signed namespace is not the one it was read from; mediatore-guest does
  not link `sandboxpolicy-bundle` yet (roadmap 00 phase 3). The docs were ahead of the code.
- `CLAUDE.md`, `CONTRIBUTING.md`, `.claude/rules/rust-style.md`, the `Makefile` comment on
  `bundle-deps-check`: the guest *will* link the bundle crate, not *does*.
- `docs/architecture.md`, `docs/src/guides/install.md`: notaio is the only status writer, and the
  username to change when notaio runs in another namespace.

## [2026-11-21 21:30] - Attest without a storage record; roadmap 00's rule for a superseded bundle

**Author:** Erick Bourgeois

### Changed
- `.github/workflows/build.yaml`: the attest job passes `create-storage-record: false`. Since the
  provenance action's v4, every release warned that `artifact-metadata: write` was missing and
  that no storage record could be made. The record is optional org metadata and verifies nothing;
  the provenance attestation is pushed as before, and the token gains no permission.
- `.github/community/00-m3-integrate-the-consumers.md`, phase 2, "At every mint": a bundle
  superseded since the claim is followed only while the running sandbox is still inside the new
  version (it would be admitted under it, and nothing it was rendered with is wider), otherwise
  the mint is refused and the sandbox claimed again. Decided 2026-11-21, replacing both the
  roadmap's "always refuse" and mediatore's "always follow"; mediatore builds it, shadow first.

## [2026-11-21 21:00] - Roadmap 00 audited against the mediatore tree

**Author:** Erick Bourgeois

### Changed
- `.github/community/00-m3-integrate-the-consumers.md`: status and phases 2 to 4 rewritten from
  the state of mediatore on 2026-11-21 (its ADR-0030, ADR-0053, ADR-0054, ADR-0055). Not complete:
  phase 2 is built behind switches that default to shadow and proven on kind, not enforced live;
  eligibility by group is not built; phase 3 has not started (D4 undecided); D6 is open. One box
  closes (the broker's egress and rules kept only as a ceiling). A "what is left" list is added.
- Recorded, not decided here: mediatore follows a superseded bundle's newer version at a mint
  where this roadmap said the mint is refused.
- `ROADMAPS.md`: the roadmap 00 row and the M3 paragraph say the same.

## [2026-11-21 20:30] - One changelog: the root CHANGELOG.md is removed

**Author:** Erick Bourgeois

### Removed
- `CHANGELOG.md` at the repository root. This file is the project's only changelog from now on;
  release notes are written for each GitHub release from its entries.
- The documentation site's Changelog page (`docs/src/changelog.md`) and its `nav` entry: it only
  pulled in the root file.

### Changed
- `.claude/rules/documentation.md`, `CONTRIBUTING.md`, `GOVERNANCE.md`, `README.md`,
  `docs/src/developer/index.md`, ADR-0009 (one sentence) and the `release-version` warning in the
  `Makefile` no longer point at the root file.

## [2026-11-21 19:30] - notaio writes only its own ConfigMaps (ADR-0003 amended, security audit)

**Author:** Erick Bourgeois

### Fixed
- `crates/notaio/src/adopt.rs` (new): `is_foreign` says whether a ConfigMap holding a bundle's
  name is not notaio's (no `app.kubernetes.io/managed-by: notaio` label). `reconcile.rs` and
  `audience.rs` ask it before publishing: a foreign ConfigMap is never applied to, its
  `bundle-version` and `bundle-digest` annotations are never read, and the object reports
  `Ready=False, BundleNameTaken`, requeued every 300 seconds. Before, the forced server-side apply
  and the controller owner reference took over any ConfigMap named `<policy>-bundle` or
  `<audience>-audience`, and deleting the policy garbage-collected it.

### Changed
- ADR-0003 amended (decision 1). `docs/architecture.md`, the troubleshooting table.
- The label key and value are constants in `adopt.rs`, used where the ConfigMaps are built.

- `Makefile` `kind-deploy`: `rollout restart` after the image is set. The tag is always
  `local-dev`, so on an existing cluster the pods kept the previous build; the new e2e first ran
  against the old controller that way (and failed, which is the behaviour it exists to catch).

### Tests
- Five unit tests in `adopt_tests.rs`; `e2e_bundle`:
  `a_bundle_name_held_by_another_configmap_is_left_alone` (content kept, no owner, no annotation,
  survives the policy's deletion).

### Threat model pass
- `docs/threat-model-mapping.md`: one Implemented row added, stamp names ADR-0003 as amended.
  No other row moves; `docs/air/` unchanged.

## [2026-11-21 18:30] - Release prep v0.7.0

**Author:** Erick Bourgeois

### Changed
- `Cargo.toml`, `Cargo.lock`: workspace version 0.7.0 (from 0.5.2; v0.6.0 and the two release
  candidates were stamped from their tags, with no prep commit).
- `CHANGELOG.md`: `[0.7.0] - 2026-11-21` (SandboxAudience and the fixes since rc.1) and, late,
  `[0.6.0] - 2026-11-17` for the jiff change that release shipped.
- `docs/src/guides/consume-a-bundle.md`: a consumer compares the bundle's signed namespace and
  name with what it looked up (from the security audit); a section on audience bundles; the
  example uses `Timestamp::now()`, left at `Utc::now()` by the jiff change. Rustdoc on
  `floor::accept` says the same.
- This file and the `cargo-quality` skill no longer name the maintainer's build hosts
  (`rules/no-real-infrastructure.md`).

## [2026-11-21 17:00] - Hardening from the security audit: version limit, OpenBao body, release version

**Author:** Erick Bourgeois

### Fixed
- `crates/sandboxpolicy-core/src/evaluate.rs`, `audience.rs`: `next_version` replaces
  `prev + 1`. At or past `jcs::MAX_SAFE_INTEGER` nothing is issued (`Ready=False,
  VersionExhausted`, status keeping the last version and digest). The previous version is the
  higher of status and the ConfigMap annotation, which is not notaio's alone to write; `u64::MAX`
  wrapped to 0 in a release build. One test each for a policy and an audience.
- `crates/notaio/src/openbao.rs`: the request and the reading of its body sit under one
  `REQUEST_TIMEOUT`, and the body is capped with `http_body_util::Limited` at
  `MAX_RESPONSE_BYTES` (64 KiB). The timeout used to end at the response head. No new dependency.
  Not covered by a test: the module's tests have no HTTP server to stall.
- `.github/workflows/build.yaml`: the release version reaches `make` through `env:`, never
  interpolated into `run:`. `Makefile`: `check-version` (reads `$VERSION` from the environment)
  is a prerequisite of `release-version`, `set-version`, `release-manifest` and
  `release-manifest-sign`; `check-version-test` in the Test job.

## [2026-11-21 12:00] - ADR-0020 amended: collision standing, header values (security audit)

**Author:** Erick Bourgeois

### Fixed
- `crates/sandboxpolicy-core/src/audience.rs`: `refused_in_collision` now decides on a
  `Standing` (new, changed, stable) per audience, replacing the stable-or-not test. A new audience
  always loses and refuses nothing; changed gives way to stable; two changed audiences of
  different tenants are decided by the namespace (platform kept), of the same kind both refused.
  Before, any two non-stable audiences refused each other, so a refused newcomer parked on a
  platform audience's name or endpoint made that audience's next edit a finding, and it was no
  longer re-signed. Found in the 2026-11-21 security audit, in v0.7.0-rc.1 only.
- `header_value_problem`: `extraHeaders[].value` must be non-empty printable ASCII (inner spaces
  and tabs allowed, no leading or trailing whitespace), under `L026`. Names alone were checked.

- `crates/sandboxpolicy-core/src/lint.rs`: `rule_problem` (L017 on audience and definition
  rules, L007 on direct egress) refuses a rule path a consumer would refuse as a request, the same
  rule as mediatore's `path_is_canonical`, confirmed with the mediatore session: dot segments,
  inner empty segments, backslash, control characters, malformed escapes and escapes decoding to
  `.`, `/`, `\\` or a control character. A dead rule before, never a widening, but L028's coverage
  check read it as inside a wider rule. Two tests in `lint_tests.rs`.

### Changed
- ADR-0020: `Amended: 2026-11-21`, decision 6 rewritten as a table of standings with the reason,
  decision 7 extended to values, two threat rows; status no longer says "Not implemented".
- `L026`'s meaning in `RULES`, the rustdoc table and the reference page.
- `docs/src/concepts/audiences.md`, `docs/architecture.md`, CRD description of `HeaderPair.value`.

### Tests
- 8 new unit tests in `audience_tests.rs`, among them every pair of standings, cross-tenant and
  not, asserting at least one side of a collision is refused.

### Threat model pass
- `docs/threat-model-mapping.md` re-stamped 2026-11-21: the team-audience row now cites the
  standing rule and the value check. No other row moves; `docs/air/` unchanged.

## [2026-11-20 16:00] - ADR-0020 SandboxAudience: signed audience definitions

**Author:** Erick Bourgeois

### Added
- ADR-0020 accepted, with the deciders' resolutions recorded: the platform audience namespace is
  `NOTAIO_AUDIENCE_PLATFORM_NAMESPACE` with no default and audiences are opt-in through it; no team
  audience through a mapped-namespace policy; a grant's endpoints are a subset of the definition's
  and an empty list reaches nothing (empty rules mean the definition's); `secretRef` and `caRef`
  carry no namespace field; a collision refuses the newcomer (an audience is stable when its
  published digest is its current content).
- `sandboxpolicy-types`: `AudienceDefinition` and its parts (`Backend`, `Placement`,
  `CredentialFormat`, `CredentialSource`, `SecretRef`, `OpenBaoSource`, `HeaderPair`,
  `DelegatedRefresh`, `ConfigMapKeyRef`). No new dependency.
- `sandboxpolicy-api`: the `SandboxAudience` CRD (`v1alpha2`, namespaced, status like a policy's).
- `sandboxpolicy-bundle`: the audience bundle (`audience.rs`): payload type
  `application/vnd.firestoned.sandboxaudience.bundle.v1+json`, schema `audience-bundle/v1`, golden
  canonical bytes and digest pinned in `tests/audience.rs` (the digest checked independently with
  `shasum`). `verify_audience_bundle` shares the signature and window checks with `verify_bundle`
  (`check_signature`, `check_window`). `floor::accept_audience`, `next_audience_floor`, with an
  audience's floor entry named `audience/<name>` so it never meets a policy's in one store.
- `sandboxpolicy-core`: `audience.rs` with `AudienceCatalog`, `lint_audience` (L017, L022 to L026,
  L029), `lint_grants` (L027, L028), `lint_definitions_against_ceiling` (L014),
  `evaluate_audience`, and `rule_covers` (the same segment glob rule as mediatore's
  `glob_covers`). `EvalInput.audiences`: `None` keeps every earlier behaviour. 49 unit tests, 5
  integration tests in `tests/evaluate.rs`.
- `notaio`: the audience reconciler (`src/audience.rs`), started only with the namespace set; one
  audience cache shared with the policy reconciler and awaited before either runs; policies
  re-evaluated when an audience they grant changes, audiences when one they collide with changes.
  `BundleSigner::sign_audience`, `events::audience`. `notaio lint --platform-audience-namespace`,
  input codes I010 and I011.
- Conformance kit for audience bundles: `conformance/audience-vectors.json` (15 vectors) and
  `audience-bundle-v1.schema.json`, `conformance::audience_suite` and `run_audience`; an optional
  `minVersion` on `Vector` (skipped when unset, so `vectors.json` keeps its shape) and one new
  policy vector, `refused-audience-envelope`. The reference page lists both suites.
- `examples/sandboxaudience-team.yaml`, `examples/sandboxaudience-platform.yaml`; `make
  policy-lint` lints with audiences on.
- e2e: `e2e_audience` (`make kind-e2e-audience`, a matrix row, `E2E_SUITES`): published and
  verified against OpenBao's key, refused as a policy bundle, kept on an L022 finding, deleted with
  its object; an undefined grant publishes nothing. `e2e_rbac` covers `sandboxaudiences`;
  `e2e_admission` refuses a direct `SandboxAudience`.
- Docs: `docs/src/concepts/audiences.md`, `docs/src/guides/bring-your-own-credential.md`,
  `docs/src/guides/platform-audiences.md`; `docs/architecture.md` (the CALM step, deferred as for
  every ADR until the model is bootstrapped); reference pages regenerated.

### Changed
- `deploy/controller/10-rbac.yaml`: read on `sandboxaudiences`, `patch` on its status, `update` on
  its finalizers. Nothing else; notaio still reads no Secret.
- `deploy/admission/gitops-only.yaml` covers `sandboxaudiences`.
- `deploy/controller/20-deployment.yaml`: the variable, commented out.
- `crates/sandboxpolicy-core/src/lint.rs`: `endpoint_problem` factored out of L017 for reuse;
  `RULES` has 29 entries.

### Threat model pass
- Full pass over `docs/threat-model-mapping.md`, stamped against ADR-0001 to ADR-0020: eight rows
  updated, two Implemented rows added (signed definitions; a team's audience kept to its namespace
  and credential), one Supported row (mediatore resolves credentials, ADR-0055), two Not covered
  rows (the Secret behind a reference; no per-audience metrics). `docs/air/mapping.yaml` ri-29 and
  mi-23 moved; `make air` regenerated `docs/framework-mapping.md`. `registry.rs` and `builtin.rs`
  unchanged (no knob or profile change).

## [2026-11-17 09:05] - chrono replaced by jiff across the workspace (#94)

**Author:** Erick Bourgeois

### Changed
- Every crate manifest: `chrono` out, `jiff` 0.2 in (`default-features = false`, `std` and
  `serde`); `schemars` on `jiff02` instead of `chrono04` in `sandboxpolicy-api` and
  `sandboxpolicy-bundle`. `sandboxpolicy-types` had declared `chrono` and `chrono04` without using
  either; both dropped. `jiff` was already in the tree through `k8s-openapi` 0.29, so the lock
  loses `chrono` and gains nothing.
- Public API: `chrono::DateTime<Utc>` is `jiff::Timestamp` and `chrono::Duration` is
  `jiff::SignedDuration` on `BundleMeta`, `VerifyOptions`, `FloorEntry`, the conformance and
  evaluation contexts, `KnobGrantSpec.expires_at`, the status fields and the CLI's `--now`.
- `crates/notaio/src/leader.rs`, `metrics.rs`: the `MicroTime` and `Time` bridges are gone; the
  wrapped `Timestamp` is the same type now.
- `crates/sandboxpolicy-core/src/lint.rs`: a grant's day limit is added as hours
  (`HOURS_PER_DAY`), since `jiff` reserves calendar days for zoned time.
- Error and lint messages print a timestamp as `2026-11-15T16:46:49Z` rather than
  `2026-11-15 16:46:49 UTC`.
- `crates/sandboxpolicy-bundle/tests/fixtures/bundle-v0.5.2.json`, `tests/bundle.rs`: a bundle
  signed by v0.5.2 with sub-second `issuedAt` and `notAfter`, as that release wrote them, still
  verifies. Written first, green on v0.5.2, green after the swap.

### Why
Consolidation on the one time library the Kubernetes types already use, ahead of mediatore's own
move (firestoned/notaio#94). No contract changes: the DSSE signature is checked over the raw
payload bytes before the payload is parsed, `BundleContent` carries no timestamp, and both
libraries read and write RFC 3339 with an offset, so bundles, Grants and statuses written by
either build read under the other. `make manifests-check` shows no CRD drift (`format: date-time`
on both). `make deny` passes; the dependency is not new to the tree.

### Impact
- [x] Breaking change (public API of the `sandboxpolicy-*` crates: a minor bump)
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-11-08 20:30] - Release prep v0.5.2; ADR-0016 and ADR-0019 amended (one v4)

**Author:** Erick Bourgeois

### Changed
- Cargo.toml and Cargo.lock at 0.5.2; CHANGELOG [0.5.2] (L021).
- Recorded late: #89 also carried the amendments to ADR-0016 and ADR-0019. One bundle schema
  change, v4, with `resources` and `sources` both optional, built once by whichever ADR is built
  first; ADR-0019's lint code becomes `L022` (L021 is taken); ROADMAPS M4 says so.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-08 19:40] - L021: ceilings that disagree about an audience; lint tests moved

**Author:** Erick Bourgeois

### Added
- `sandboxpolicy-core/src/lint.rs`: `lint_classifications`, `L021` (a warning): an audience two
  ceilings classify differently, unset counting as `Internal` (ADR-0017 decision 2), reported on
  each ceiling that takes part with the audience's index and the other ceilings named. RULES and
  the module table have it; `make docs-reference` regenerated `lint-rules.md`.
- `crates/notaio/src/lint.rs`: `lint_sources_with` runs it over every ceiling in the inputs;
  `describe_ref` factored out of `describe`. The `Diagnostic.code` doc names the current ranges.
- Tests first: four in `lint_tests.rs` (both warn, unset is Internal, agreement is clean, the
  audience index), two in `tests/lint.rs` (end to end, a warning that does not fail the run);
  `rules_table_matches_the_documented_codes` counts 21.

### Changed
- The inline `mod tests` of `sandboxpolicy-core/src/lint.rs` moved to `lint_tests.rs`, as
  `rules/testing.md` asks; no test changed.
- Docs: ADR-0017 (the follow-up is done), concepts/ceilings, threat model mapping (the Lab
  audiences row), CHANGELOG Unreleased.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-08 19:00] - ADR-0019 proposed: resource sizes for K2.4

**Author:** Erick Bourgeois

### Added
- `docs/adr/0019-resource-sizes.md` (Proposed): sizes per K2.4 step in `Ceilings` (proposed
  1/2/4 vCPUs, 2/4/8 GiB, 10/20/40 GiB, pending the model), optional `spec.resources`, lint
  `L021`, bundle schema v4 with consumers first, enforcement by whatever claims the VM.
- Site stub, ADR index row, mkdocs nav; ROADMAPS M4 K2.4 points at it.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-08 17:40] - Fix: the half-lifetime refresh could slip to notAfter

**Author:** Erick Bourgeois

### Fixed
- `crates/notaio/src/refresh.rs` (new, in the library so it is tested): `is_stale` (moved out of
  `reconcile.rs`) and `next_check`, the wait before reconciling an unchanged policy: the
  evaluator's requeue or the moment the bundle turns stale plus one second, whichever is first,
  never under one second. `reconcile.rs` uses both.
- Found installing v0.5.0 on the test cluster: reconciles ran every 450 s from the restart, one at
  655 s left found the hour bundle fresh, the next re-signed it with 205 s left, and
  `NotaioBundleNearExpiry` fired. Present in every earlier release (with an hour, a refresh could
  come seconds before expiry just the same).
- Tests first in `refresh_tests.rs`: the incident, every phase from 451 s to four lifetimes left,
  the evaluator's earlier grant requeue, no bundle, an already stale bundle. Against the old
  behaviour three of nine fail.
- Docs: CHANGELOG Unreleased (Fixed), ADR-0018 note, architecture ("Refreshes on time"). No
  threat model change: the T5.4 row already says what the controller now does.

### Impact
- [ ] Breaking change
- [x] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-07 22:30] - ADR-0018 implemented: a 15-minute, bounded bundle lifetime

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/src/cli.rs`: `bundle_lifetime_seconds`, default 900, bounds 300 to 86400, an error
  naming the variable otherwise; `main.rs` uses it with `?` (exit 1). Tests first in
  `cli_tests.rs` (default, in bounds, `15m`, empty, 0, negative, 299, 86401, `1e3`).
- `metrics.rs`: `notaio_bundle_lifetime_seconds` (test in `metrics_tests.rs`; the reference test
  now counts 14 families). `lint.rs` uses the controller's default instead of its own 3600.
- `deploy/controller/20-deployment.yaml`: 900, with a comment on the trade.
- Docs: glossary, bundle concepts, key rotation, observability (alert relative to the lifetime),
  architecture; ADR-0018 Accepted, ADR-0002 open decision 3 marked resolved; threat model pass
  stamped against ADR-0018 (the T5.4 row says the bound); ROADMAPS M4.

### Impact
- [x] Breaking change (the default lifetime, and a refused out-of-bounds value)
- [x] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-07 21:30] - ADR-0018 proposed: tightening latency

**Author:** Erick Bourgeois

### Added
- `docs/adr/0018-tightening-latency.md` (Proposed), resolving ADR-0002 open decision 3: the
  tightened path is already immediate for watching consumers (the watch is the pushed floor); the
  lifetime bounds only a policy that turns invalid and non-watching consumers. Default 900 s,
  bounded 300 s to 86400 s, a bad `NOTAIO_BUNDLE_LIFETIME_SECONDS` a startup error (today it
  silently falls back, and accepts 0 or negative), `notaio_bundle_lifetime_seconds` and a
  `NotaioBundleNearExpiry` relative to it (the fixed 600 s would always fire under a 20-minute
  lifetime). Stub, index row, nav; ROADMAPS M4.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-07 20:30] - Release prep for v0.4.0

**Author:** Erick Bourgeois

### Changed
- `Cargo.toml` and `Cargo.lock` at 0.4.0 (`make set-version`, `cargo update --workspace`; `--locked`
  holds).
- CHANGELOG.md: `[0.4.0]`. The dashboard, L020 and honorLabels entries had been inserted under
  `[0.3.6]` (the first `### Added` after Unreleased was 0.3.6's); moved to 0.4.0.

### Why
L020 can stop an unclassified Lab policy publishing, so a minor version.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-07 19:00] - ADR-0017 implemented: audience classification and L020; honorLabels

**Author:** Erick Bourgeois

### Added
- `crates/sandboxpolicy-api/src/ceiling.rs`: `CeilingAudience.data_scope: Option<DataScope>`
  (additive, v1alpha2). `make manifests` regenerated the CRD and the reference.
- `crates/sandboxpolicy-core/src/lint.rs`: `L020` in `lint_exposure`, which now takes the profile's
  `DataScope`; `evaluate.rs` passes it. Unset is `Internal`.
- Tests first (`tests/evaluate.rs`): a lab policy refused for an unset and an `Internal` audience
  and published for a `LabOnly` one; only the unclassified audience reported; standard policies
  unaffected; `RULES` is 20. `covering_ceiling` classifies lab policies' audiences `LabOnly`.

### Changed
- ADR-0017 Accepted with the strict reading (every audience of a Lab policy), decided in review.
- Threat model full pass, stamped 2026-11-07 against ADR-0001 to ADR-0017: "Lab audiences only"
  moved from Not covered to Implemented (L020); the L003 row notes L020; the distribution and
  downgrade rows say what mediatore built in phase 1 (they still said "not built"); the dashboard
  row points at `deploy/monitoring/`. `docs/air/mapping.yaml` ri-1, mi-4 (stale M2 gap) and mi-6;
  `make air`. `docs/architecture.md` known gaps.
- Concepts: ceilings (a Lab audiences section, the L020 row, upgrade advice), knobs (L003's
  construction is now checked). `examples/exposureceiling-agent-default.yaml` sets `dataScope`.
- Observe notaio: `honorLabels: true` on the PodMonitor (found on the test cluster: the policy
  namespace label was replaced by the pod's).

### Impact
- [x] Breaking change (a LabOnly policy whose ceiling is not classified stops publishing)
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-07 17:30] - ADR-0017 proposed (audience classification); a Grafana dashboard

**Author:** Erick Bourgeois

### Added
- `docs/adr/0017-audience-classification.md` (Proposed): `dataScope` on each ExposureCeiling
  audience, unclassified meaning `Internal`, and lint `L020` refusing a `LabOnly` policy that
  exposes a non-lab audience. Calls out its one interpretation of the model (every audience of a
  Lab policy, because `L003` assumes it) for review. Stub, nav entry, and index rows for ADR-0015
  to ADR-0017 (the index lacked 0015 and 0016).
- `deploy/monitoring/`: `notaio-dashboard.json` (18 panels, Health, Policies, Bundles, Relaxations)
  and a kustomization that wraps it in ConfigMap `notaio-dashboard` with the sidecar label.
  Every query ran against a live Prometheus; Grafana 12.2 imports it.
- Observe notaio: a Dashboard section. ROADMAPS M4: knob dashboard done, audience classification
  proposed.

### Why
M4: the threat model's "lab audiences only" for K5.2 and K5.3 was not enforceable, and the knob
dashboard was a sentence in the guide.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-07 16:00] - v0.3.6 verified: the lost VEX attestation, and a new zlib CVE

**Author:** Erick Bourgeois

### Changed
- `.github/workflows/build.yaml`: `scan` needs `attest`. Both pushed an attestation to the same
  fallback referrers index at once (GHCR has no referrers API; each reads and rewrites the index);
  in v0.3.6 the provenance, pushed half a second after the VEX attestation, dropped it.
- `.vex/CVE-2026-76844.json` and its evidence (libz.so.1 not linked): `make vex-evidence` holds for
  21 statements; a Grype scan of v0.3.6's image with them reports no High.
- CHANGELOG.md Unreleased.

### Why
Verifying v0.3.6: everything else held (checksums; Cosign on the manifests, both CLI binaries and
the image; SLSA provenance; SBOM 194 crates with kube 5.0.0; the amd64 binary reports 0.3.6, stamped
from the tag without a prep commit); the VEX attestation was missing and a new High was unaccounted.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-07 14:00] - After v0.3.5 and v0.3.6: the version and changelog they shipped without

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.3.6; `CHANGELOG.md`'s Unreleased split into `[0.3.6]` (L019, kube 5, the
  docs status labels) and `[0.3.5]` (the CLI release assets), each noting that it was released
  without a prep commit; the install guide's `V=` is 0.3.6.

### Why
Both releases were cut from `main` by tag alone (ADR-0009, the tag is the version); the artifacts
are stamped right, the source on `main` catches up here.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-07 10:00] - L019, and roadmap 00 caught up with mediatore's gateway

**Author:** Erick Bourgeois

### Changed
- `crates/sandboxpolicy-core/src/lint.rs`: `L019`, a warning on a direct egress entry with method
  and path rules or a wildcard host, which mediatore's gateway does not serve (its ADR-0030
  decision 3). Test `l019_warns_of_direct_egress_the_gateway_cannot_enforce`; the rule count 19;
  the lint reference regenerated.
- Docs: ADR-0013 decision 5 amended; glossary, the policy concept, write-a-policy.
- Roadmap 00: D5 decided (mediatore ADR-0028), phase 4 largely built, phase 2's first slice
  (ADR-0030, behind a switch); `ROADMAPS.md` row 00. CHANGELOG.md Unreleased.

### Why
mediatore's gateway serves an exact host:port only; notaio accepted entries it would never serve,
and an author would learn that only from mediatore's `P007`.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-06 17:00] - ADR-0016 proposed: the source of a policy in its bundle

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0016-source-in-the-bundle.md` (new, Proposed, with its site wrapper and nav entry): an
  annotation `sandbox.firestoned.io/source` the delivery pipeline stamps, copied unverified into
  `content.sources`; no GitOps API; a `bundle/v4` that consumers accept before notaio emits it,
  because `verify_bundle` recomputes the digest from parsed content and refuses an unknown field.
  Waits for a consumer that needs it (roadmap 00 D6).
- Roadmap 03's "later" note points to it.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-06 16:00] - Docs: a status on every page (M5)

**Author:** Erick Bourgeois

### Changed
- `docs/mkdocs.yml` `extra.status` (implemented, partial, planned), icons in
  `docs/src/stylesheets/extra.css`; front matter `status:` on every content page (changelog and
  governance unlabelled, ADRs keep their own Status); `docs/snippets/status-consumers.md`, the
  note at the top of the seven pages that describe consumers enforcing.
- `crates/notaio/src/reference.rs`: generated pages open with `status: implemented` front matter;
  `reference_tests.rs` checks it; `docs/src/reference/` regenerated.
- `docs/src/concepts/bundle.md`: signed delivery is ADR-0015, no longer "planned in roadmap 03".
- `ROADMAPS.md` M5: status labels done; CHANGELOG.md Unreleased.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-06 12:00] - kube 5, k8s-openapi 0.29: the dependency upgrade validated

**Author:** Erick Bourgeois

### Changed
- `cargo upgrade --incompatible` (Erick): kube 4 to 5, k8s-openapi 0.28 to 0.29, tokio 1.54.1,
  serde_json 1.0.154, schemars 1.2.3, serde-saphyr 2.1.1, prometheus-client 0.25.2; 97 lock entries.
- `crates/notaio/tests/e2e_smoke.rs`, `e2e_openbao.rs`: `Deployment.spec` is no longer an `Option`
  in k8s-openapi 0.29 (three uses). No library or controller code changed.
- CHANGELOG.md Unreleased.

### Why
Validated on the build host: lint, 159 tests, manifests/docs-reference/conformance/bundle-deps/license
checks, `cargo deny`, the release build with an SBOM of 194 Rust crates (kube 5.0.0 among them) and
all 20 VEX statements holding, and `make kind-e2e-ci E2E_SUITE=all` on a fresh kind cluster: install,
bundle, rbac, admission, leader, openbao, all passed.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-03 09:00] - v0.3.4: release prep, and a release workflow that survives without it

**Author:** Erick Bourgeois

### Changed
- Workspace version 0.3.4; `CHANGELOG.md` gains its `[0.3.4]` section (the `TrustedKeys` change,
  ADR-0015's guide, the workflow fix); the install guide's `V=` is 0.3.4.
- `make release-version` (new): the tag's version into `Cargo.toml`, `Cargo.lock` matched offline,
  a GitHub warning when `Cargo.toml` said otherwise; the build workflow calls it after installing
  the toolchain, in place of `set-version`. `--locked` stays. `make set-version-test` checks the path
  with `--locked`. ADR-0009 decision 6 amended (the tag is the version); the developer page says
  how to cut a release.

### Why
v0.3.4 was released before its release-prep commit; the stamp moved `Cargo.toml` but not
`Cargo.lock`, and the `--locked` binary build refused. Nothing was published; the release is
recreated on this commit.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [ ] Documentation only

## [2026-11-02 17:00] - Roadmap 00 phase 1 done (in mediatore)

**Author:** Erick Bourgeois

### Changed
- Roadmap 00: phase 1's three items ticked from mediatore's finished roadmap 09 (the Role, the
  reflector and floor, the shadow comparison and its live tier); status: phase 2 next.
- `ROADMAPS.md`: row 00 and the M3 prose.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-02 16:00] - Roadmap 03 done: ADR-0015 and the signed-delivery guide

**Author:** Erick Bourgeois

### Changed
- `docs/adr/0015-signed-policy-delivery.md` (new, with its site wrapper): two artifacts, two
  signers, Flux verification; the accepted risk of CI-signed policies; the Cosign 3 details.
- `docs/src/guides/deliver-signed-policies.md` (new): the CI job, Security's procedure (and the
  `COSIGN_KEY` warning), the `OCIRepository` objects, the cutover without a gap, the checks.
- Threat model mapping (T8.2) and the security page's guarantees; the guides index and nav;
  CHANGELOG.md Unreleased.
- Roadmap 03 done; `ROADMAPS.md` row 03.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-02 13:30] - Roadmap 03 phase C: the test cluster on the signed artifacts

**Author:** Erick Bourgeois

### Changed
- Roadmap 03 phase C: the cutover (three steps, no object recreated), the tightening through the
  signed path, the CI-only ceiling refused until Security signed it, smoke after each; the
  signing-config follow-up done; the unsigned `policies` push refused. Phase C done.
- `ROADMAPS.md` row 03.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-02 11:00] - Roadmap 03 phase B done

**Author:** Erick Bourgeois

### Changed
- Roadmap 03: phase B's Security item and documentation item done (Security's key pair, the
  first `ceilings` signed and verified); the `COSIGN_KEY` and deprecated-flag findings recorded;
  a follow-up for a no-transparency-log signing config. Status: phase C next.
- `ROADMAPS.md` row 03.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-02 10:00] - Roadmap 03: D1 and D2 decided, CI publishes

**Author:** Erick Bourgeois

### Changed
- Roadmap 03: D2 decided (CI signs both artifacts keyless; Security also signs `ceilings` with a
  key; Flux requires CI's on `policies`, Security's on `ceilings`); phase B's CI item done, the
  Security item's script and procedure written.
- `ROADMAPS.md` row 03.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-01 23:45] - Roadmap 03 phase A: Flux verifies cosign signatures as assumed

**Author:** Erick Bourgeois

### Changed
- Roadmap 03 phase A ticked, run on the test cluster's Flux v2.10.0 in a throwaway namespace:
  key-signed artifacts verify in both Cosign formats with no transparency log; unsigned, wrong-key
  and a tag moved to an unsigned digest are refused with the reason, the applied content kept. The
  Cosign 3 finding: `--tlog-upload=false` needs `--use-signing-config=false` (or a signing config
  with no transparency log); sign by digest.
- `ROADMAPS.md` row 03 in progress.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

## [2026-11-01 23:00] - Roadmap 02 done: the test cluster's OpenBao unseals itself

**Author:** Erick Bourgeois

### Changed
- Roadmap 02 phase B ticked and the roadmap done: on the test cluster, a backup, the migration by
  `setup.sh` (21 s; transit key and public key unchanged), `check.sh` clean, a deleted pod unsealed
  in 4 s, notaio's next re-sign through it, smoke 3 of 3. Commands and names in the policy
  repository's `TESTING.md`.
- `ROADMAPS.md`: row 02 done, the M3 prose; roadmap 01's OpenBao risk resolved.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config change only
- [x] Documentation only

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
  `.vex/` is a build-relevant path. All run end to end on the build host against the main image: 19
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
  in the Build workflow. Proven on the build host: the same image's Syft SBOM goes from 0 to 200 crates.
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
