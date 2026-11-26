<!-- Copyright (c) 2026 Erick Bourgeois, notaio -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap 05: bundle schema v4, one migration, consumers first

- **Status:** Planned: opened 2026-11-26. Nothing built.
- **Milestone:** M4 in [`ROADMAPS.md`](../../ROADMAPS.md)
- **Related:** ADR-0002 (the bundle contract, amended by this), ADR-0005 (canonical form),
  ADR-0008 (the consumer's floor), ADR-0013 (the last schema change, v2 to v3), ADR-0014 (how
  consumers take the crates), ADR-0016 (`sources`), ADR-0019 (`resources`), ADR-0023 decision 7
  (`includes` and `from`), ADR-0024 (`subjects`), ADR-0025 (a shared ceiling, open question 1 of
  ADR-0024), roadmap 04 phase 4. In mediatore: ADR-0026 (verification in shadow, the pinned tag),
  ADR-0054 decision 7 and accepted risk R-54 (eligibility waits on this), ADR-0061 (a running
  claim follows a new bundle), ADR-0066 (eligibility by group, proposed),
  `docs/design/policy-subjects.md`. Threat model K7.2, T5.4.

## 1. Goal

Every bundle notaio signs says who may use it, and every consumer reads that from the signed
bundle alone. Four ADRs add members to the bundle, and each says the same thing: one `bundle/v4`,
built once, taken by consumers before notaio emits it. This roadmap is that one migration.

What it closes:

- **Eligibility by group (K7.2) is enforced nowhere.** `spec.subjects.groups` is linted and then
  dropped at compile, so whoever may create a sandbox in a namespace may name any policy that
  namespace draws from (`docs/threat-model-mapping.md`, "Not covered yet"; mediatore R-54).
  ADR-0024 signs the list; mediatore's ADR-0066 checks it.
- **A composite's bundle cannot say which include a grant came from** (ADR-0023 decision 7), so
  mediatore's `PolicyConformant` and audit line cannot name it (roadmap 04 phase 4).
- **`resources` (ADR-0019) and `sources` (ADR-0016) get their place in the schema** without a
  second migration, even though neither is filled yet.

## 2. What we know from the code

Surveyed 2026-11-26 in notaio, mediatore and banlieue:

- **The verifier checks the schema exactly**, and checks the payload type before it: a v0.8.0
  consumer refuses a v4 envelope as `PayloadType` (`crates/sandboxpolicy-bundle/src/verify.rs`).
  So a lockstep mistake, notaio emitting v4 before its consumers can read it, refuses every
  bundle; under mediatore's `policy.audiences: enforce` every claim loses its audiences.
- **The digest is recomputed from the parsed content.** A field the reader's types do not model is
  dropped on parse and the bundle fails with `DigestMismatch`. So the v3-and-v4 verifier must model
  every v4 member, `from` included, or valid v4 bundles fail.
- **`envelope()` and `signing_input()` hard-code the v3 payload type**
  (`crates/sandboxpolicy-bundle/src/dsse.rs`), and the OpenBao signer uses exactly that pair
  (`crates/notaio/src/signer.rs`). A v4 payload signed through OpenBao would go out under the v3
  type. The development signer would not show it; `e2e_openbao` would.
- **`AudienceGrant`, `EgressRule` and `ToolRef` are also the CRD's spec types**
  (`crates/sandboxpolicy-api/src/policy.rs`). A `from` field on them would appear in the CRD,
  where authors could set it, and would break seven mediatore test sites that build those structs
  field by field.
- **The controller already has what v4 needs.** `compose` returns each named entry's origin and
  the includes, name-sorted; `status.includes` is written from them. `compile_content` re-sorts
  the lists and loses the origins, so it must carry them through the sort.
- **The floor is schema-agnostic** (`floor.rs` reads the policy and `meta` only), and the version
  keeps counting: the v4 digest always differs from v3, so the cutover is one new version for
  every policy (ADR-0024 decision 4).
- **Only mediatore consumes bundles.** Only its broker (`serve`) reads them, through
  `mediatore-policy`, pinned to tag `v0.8.0`. banlieue does not link the crates. mediatore reads
  `policy`, `audiences`, `egress` and `lease`, and ignores `profile`, `knobs`, `tools` and
  `budgets`.
- **The docs promise a release-notes callout for a schema change** (`guides/install.md`,
  `guides/consume-a-bundle.md`), and nothing produces one: the v3 release's notes were GitHub's
  generated list of pull requests.

## 3. Decisions

**D1. One migration ADR.** *Recommended:* ADR-0026, "Bundle schema v4 and its rollout", amending
ADR-0002. It owns what no field ADR does: the field list as a whole, the pairing of payload type
and schema, the dual verifier, how the producer switches, when v3 is dropped, and the release
notes. ADR-0024 is accepted with it, its open question 1 left to ADR-0025 (a ceiling over groups
changes no bundle field). ADR-0016 and ADR-0019 stay Proposed: their fields are in the schema,
filled when each is built. *Alternative:* an ADR-0002 amendment alone; it would hide a rollout
with three releases inside a contract document.

**D2. What v4 carries when notaio first emits it.** *Recommended:*

| Member | Filled | Absent means |
| --- | --- | --- |
| `subjects { groups }` | always, sorted; `[]` means nobody (ADR-0024) | never absent in v4: `Malformed` |
| `includes [{ name, uid, contentDigest }]` | for a composite, sorted by name (ADR-0023) | not a composite |
| `from` on each audience, egress entry and tool | for an entry an include supplied | the composite's own entry |
| `resources { vcpus, memoryMiB, diskGiB }` | once ADR-0019 is built | K2.4 unchecked, as today |
| `sources { policy, ceiling, grants }` | once ADR-0016 is built | no source claimed |

Every optional member is omitted, never `null`, so an absent member does not change the canonical
form. v4 does not wait for the model's K2.4 numbers: ADR-0019's own amendment fixes the shape now
and keeps the numbers as data.

**D3. The verifier in the transition release.** *Recommended:* one `BundlePayload` whose v4
members are optional, so `verify_bundle` and `accept` keep their signatures and a v3 bundle
recomputes the same digest. `verify_bundle` accepts either payload type and then refuses:

- a payload type and a schema that do not pair (v3 with v3, v4 with v4): `Schema`;
- a v3 payload carrying any v4 member or any `from`: `Schema`, so v3 is never read as v4;
- a v4 payload without `subjects`: `Malformed`.

The payload says which it is (`schema_version()`), and `VerifyOptions` gains a switch to refuse
v3, for a consumer that enforces subjects. When v3 is dropped, `subjects` stops being optional.
*Alternative:* a frozen v3 type and an enum of the two; more precise, but it breaks every caller
twice, once to add v4 and again to drop v3.

**D4. Where `from` lives.** *Recommended:* in the bundle crate, not in `sandboxpolicy-types`:
`BundleAudience { #[serde(flatten)] grant: AudienceGrant, from: Option<String> }`, and the same
for egress and tools, with `Deref` to the grant so `content.audiences[i].name` still reads. The
CRD does not change and mediatore's test literals still compile. *Alternative:* the field on the
shared types with `schemars(skip)`; it hides the field from the conformance JSON Schema too.

**D5. How the producer switches.** *Recommended:* `NOTAIO_BUNDLE_SCHEMA`, `v3` or `v4`. In release
N it exists with default `v3`; in N+1 the default is `v4`; in N+2 it is gone. An operator turns v4
on once every consumer runs release N's verifier, and rolls back by turning it off: versions keep
counting, so a consumer on N accepts the v3 bundle that follows a v4 one, and the floor still
refuses a replay. *Alternative:* the switch is the release itself; then a rollback is a controller
downgrade.

**D6. Running claims at the cutover.** The cutover moves every policy's version once. Under
mediatore's `policy.follow`, a claim recorded before it kept its rendered policy is refused at the
first different version (its ADR-0061). *Recommended:* say so in the release notes of N+1 and in
the install guide, and leave the answer to mediatore's ADR-0066 (for example, cut over while
`follow` is `shadow`, or after such claims drain). notaio cannot see claims.

**D7. The fail-open guard checks the signed list.** *Recommended:* a fifth guard check, beside
`no-ceiling` and the others (ADR-0010): the signed `subjects.groups` equal the effective policy's
groups, sorted. It is the field that decides eligibility, so a bug that dropped or widened it
must be caught by code that is not `evaluate`. One more value in `notaio_fail_open_total`.

**D8. The release notes are written, not generated.** *Recommended:* each of N, N+1 and N+2 has a
hand-written "For consumers" section at the top of the GitHub release, and `guides/install.md`
gains an "Upgrading to bundle v4" box like the v1alpha2 one, so the promise the guides make is
kept.

## 4. Work, in order

Each phase is shippable and leaves the system no looser than before. Phases 1 to 4 make release
N; phase 6 is N+1; phase 7 is N+2.

### Phase 0: design

- [ ] ADR-0026 (D1 to D8), amending ADR-0002; ADR-0024 accepted; ADR-0016, ADR-0019 and ADR-0023
      point to it for the schema half. ADR-0024's conformance path corrected
      (`crates/sandboxpolicy-bundle/conformance/vectors.json`).
- [ ] mediatore: ADR-0066 cites notaio ADR-0024 (it says ADR-0022, which is the registry
      audience), and the switch name and P-codes in `docs/design/policy-subjects.md` follow
      ADR-0066.

### Phase 1: `L034`, alone

- [ ] `L034` (ADR-0024 decision 3): a group entry that is empty, padded, holds `*?[]{}` or repeats
      another. In the controller and `notaio lint`, on the effective spec; tests first; the
      reference regenerated. It only adds findings and needs no schema.

### Phase 2: the bundle crate verifies v3 and v4

- [ ] The v4 types (D2), the bundle-local wrappers (D4), `SCHEMA_V3`/`SCHEMA_V4` and the two payload
      types, `BundlePayload::new_v3`/`new_v4`.
- [ ] The payload type travels with the payload: `SigningInput` carries it and `envelope` takes it,
      so the OpenBao path signs v4 under the v4 type.
- [ ] `verify_bundle` with the pairing and absence rules of D3; `schema_version()`; the
      `VerifyOptions` switch to refuse v3.
- [ ] Tests first: the v3 golden bytes and digest unchanged; the v0.5.2 fixture still verifies; a
      v4 golden computed independently; each refusal of D3; a v4 payload with a `null` optional
      refused (`DigestMismatch`). `make bundle-deps-check` stays green.

### Phase 3: the conformance kit

- [ ] v4 vectors: accepted; a v3 payload under the v4 type and the reverse (`Schema`); a v3
      payload with a v4 member (`Schema`); a v4 payload without `subjects` (`Malformed`); a `null`
      optional (`DigestMismatch`). The v3 vectors stay.
- [ ] `bundle-v3.schema.json` frozen, `bundle-v4.schema.json` generated with `subjects` required;
      the generated conformance reference and `guides/consume-a-bundle.md` name both.

### Phase 4: the producer, still emitting v3

- [ ] `compile_content` carries each entry's origin through the sort and fills `includes`, `from`
      and `subjects`; with `NOTAIO_BUNDLE_SCHEMA=v3` (the default) every v4 member is left out and
      the bundle is byte for byte what release N-1 signed.
- [ ] `NOTAIO_BUNDLE_SCHEMA` read at startup, refused when it is neither value, in the environment
      reference.
- [ ] The guard check of D7, with its metric value and tests.
- [ ] e2e with `v4` set: `e2e_bundle` (the schema, the payload type, `subjects` equal to the
      fixture's groups, the version still climbing across a v3 to v4 to v3 switch),
      `e2e_compose` (`includes` equal to `status.includes`, `from` on included entries only, a
      reordering keeping the digest), `e2e_openbao` (v4 signed by transit verifies).
- [ ] **Release N**, with the notes of D8.

### Phase 5: consumers take release N

- [ ] mediatore: the pin moves to N, its conformance tests pass on both schemas, and its broker
      admits v3 and v4. Nothing enforced yet.
- [ ] mediatore: ADR-0066's eligibility check in shadow against v4 bundles on the test cluster.

### Phase 6: notaio emits v4

- [ ] On the test cluster: `NOTAIO_BUNDLE_SCHEMA=v4`, every bundle v4 within one lifetime, nothing
      refused by mediatore; then rolled back to `v3` and forward again, nothing refused.
- [ ] **Release N+1**: the default is `v4`; the notes of D8, including D6.
- [ ] Docs: `concepts/bundle.md`, the glossary, `guides/consume-a-bundle.md`, the install guide's
      upgrade box, and `docs/architecture.md` (whose bundle row still says "schema v2, frozen").
- [ ] Roadmap 04 phase 4 done (the notaio half); the threat model pass: the eligibility row and
      the include-origin row leave "Not covered yet" as far as the code allows.

### Phase 7: drop v3

- [ ] **Release N+2**: the v3 types, constants and vectors removed, `subjects` required in the
      type, `NOTAIO_BUNDLE_SCHEMA` removed; ADR-0002 amended again. Only after mediatore runs N+1.

## 5. Done when

Every bundle notaio publishes is v4 and carries the policy's groups; mediatore checks a claim's
groups against them from the verified bundle; a composite's bundle names the include behind each
grant; and no v3 code path is left in the verifier.

## 6. Risks

- **A lockstep mistake refuses every bundle.** Emitting v4 before every consumer runs release N
  refuses all of them. Mitigated by D5 (a switch an operator turns on, and back off) and D8 (the
  notes say so in all three releases).
- **The cutover moves every version at once.** Each policy re-signs once, which is load (small at
  today's scale) and, under `follow`, a cut-off for some running claims (D6).
- **A field left unmodelled fails closed, everywhere.** If the dual verifier misses a v4 member,
  every v4 bundle is a `DigestMismatch`. The v4 vectors and a v4 golden built independently of
  the types are the check.
- **`resources` is fixed before the model's numbers.** The shape is ADR-0019's own (amendment of
  2026-11-08); if the model needs a different shape, that is a v5.
- **mediatore's documents disagree with each other** (switch name, P-codes, the notaio ADR cited).
  Phase 0 settles them before code depends on either.
