# ADR-0026: Bundle schema v4 and its rollout

- **Status:** Accepted
- **Date:** 2026-11-26
- **Deciders:** Erick Bourgeois
- **Amended:** 2026-11-26 (decisions 4 and 8: the digest is taken over the content as signed, and
  a member the schema does not model is refused; found by the tests of the build)
- **Related:** Amends [ADR-0002](0002-bundle-contract.md) (the bundle contract: decisions 1, 2
  and 5); carries the schema half of [ADR-0016](0016-source-in-the-bundle.md) (`sources`),
  [ADR-0019](0019-resource-sizes.md) (`resources`), [ADR-0023](0023-composed-policies.md)
  decision 7 (`includes` and `from`) and [ADR-0024](0024-subjects-in-the-signed-bundle.md)
  (`subjects`); [ADR-0005](0005-canonical-form-rfc-8785.md) (canonical form);
  [ADR-0008](0008-consumer-version-floor.md) (the floor); [ADR-0010](0010-health-metrics-and-events.md)
  (the fail-open guard); [ADR-0013](0013-bundle-v3-routes-and-v1alpha2.md) (the last schema
  change); [ADR-0014](0014-consumer-distribution.md) (consumers pin a release);
  [ADR-0018](0018-tightening-latency.md); roadmap 05. In mediatore: ADR-0026, ADR-0061, ADR-0066.
  Threat model K7.2, T5.4.

Partly implemented (2026-11-26): the verifier and the types (decisions 1 to 4) and the
conformance kit (decision 8), in `sandboxpolicy-bundle`; the producer (decisions 5 to 7):
`NOTAIO_BUNDLE_SCHEMA`, default `v3`, and the `subjects-mismatch` guard check. Not yet: the three
releases and their notes (decision 9). Roadmap 05 carries the rest.

## Context

Four ADRs each add a member to the signed bundle and each says the same thing: one `bundle/v4`,
built once, taken by consumers before notaio emits it. None of them owns the migration itself: the
field list as a whole, how a consumer reads two schemas for a while, how the producer switches, and
when v3 ends. This ADR does.

What the code says (surveyed 2026-11-26, roadmap 05 section 2):

- The verifier compares the payload type and the schema exactly. A consumer pinned to a release
  that knows only v3 refuses every v4 bundle, and under mediatore's `policy.audiences: enforce`
  that takes every claim's audiences away.
- The verifier recomputes the content digest from the parsed content. A member the reader's types
  do not model is dropped on parse and the bundle fails with `DigestMismatch`, so a reader of v4
  must model every v4 member.
- `signing_input()` and `envelope()` always use the v3 payload type, and the OpenBao signer is
  built on that pair (`crates/notaio/src/signer.rs`).
- `AudienceGrant`, `EgressRule` and `ToolRef` are the CRD's spec types as well as the bundle's.
- The floor reads the policy and `meta` only, and the version keeps counting: a v4 digest always
  differs from the v3 one, so the cutover is one new version for every policy.
- mediatore's broker is the only consumer; banlieue does not read bundles.

## Decision

1. **The v4 content.** `SCHEMA` `sandbox.firestoned.io/bundle/v4`, payload type
   `application/vnd.firestoned.sandboxpolicy.bundle.v4+json`. v3's members are unchanged; v4 adds:

   | Member | Shape | Present |
   | --- | --- | --- |
   | `subjects` | `{ groups: [string] }`, sorted by byte order | always; `[]` means nobody (ADR-0024) |
   | `includes` | `[{ name, uid, contentDigest }]`, sorted by `name` | only on a composite (ADR-0023) |
   | `from` on each `audiences`, `egress` and `tools` entry | string, an include's name | only on an entry an include supplied |
   | `resources` | `{ vcpus, memoryMiB, diskGiB }`, unsigned integers | once ADR-0019 is built |
   | `sources` | `{ policy, ceiling, grants: { <name>: string } }`, each optional | once ADR-0016 is built |

   An optional member is omitted, never written `null`. A v4 payload with `subjects` absent is
   `Malformed`. Neither `resources` nor `sources` is filled by this ADR: their places are fixed so
   that filling them later is not a schema change.

2. **`from` lives in the bundle crate.** The bundle's entries are `BundleAudience`, `BundleEgress`
   and `BundleTool`, each the shared type flattened with an optional `from`, dereferencing to the
   shared type. The CRD does not change, and a v3 entry serialises exactly as before.

3. **Payload type and schema pair.** v3 goes with v3, v4 with v4; any other pairing is `Schema`.
   The payload type travels with the payload through signing: `SigningInput` carries it and
   `envelope` takes it, so a bundle signed out of band (OpenBao transit, ADR-0004) is enveloped under
   the type it was signed under.

4. **One release reads both.** For the release that introduces v4 and the next, `verify_bundle`
   accepts either payload type and refuses:
   - a payload type and a schema that do not pair: `Schema`;
   - a v3 payload carrying any v4 member or any `from`: `Schema`, so v3 is never read as v4;
   - a v4 payload without `subjects`: `Malformed`.

   One payload type models both, its v4 members optional, so `verify_bundle` and `accept` keep their
   signatures and a v3 bundle recomputes the digest it was signed with. The payload says which
   schema it is (`schema_version()`), and `VerifyOptions` can refuse v3, for a consumer that
   enforces subjects. No new error variant.

   *Amended:* the content digest is taken over the content exactly as signed, as a verifier in
   any language takes it, and the parsed content must re-serialise to the same bytes. Recomputed
   from the parsed types alone, a member written `null` would read as absent and verify, where a
   verifier hashing what was signed refuses it; and a member the types do not model, digested
   with it, would be dropped and the rest accepted. The first is `DigestMismatch`, the second
   `Malformed`, in every verifier.

5. **The producer switches with a setting.** `NOTAIO_BUNDLE_SCHEMA` is `v3` or `v4`, and anything
   else is refused at startup. Release N introduces it with default `v3`; N+1 changes the default to
   `v4`; N+2 removes it, with v3 itself. An operator turns `v4` on after every consumer runs N, and
   rolls back by turning it off: the version keeps climbing either way, so a consumer on N accepts
   the v3 bundle that follows a v4 one and its floor still refuses a replay (ADR-0008). Under `v3`
   the controller leaves out every v4 member and signs byte for byte what it signed before.

6. **Versions keep counting across the change.** The first v4 bundle of a policy is the next
   version, as for any digest change (ADR-0002 decision 3, ADR-0024 decision 4). notaio never
   publishes a v3 and a v4 bundle of one policy side by side.

7. **The fail-open guard checks the signed groups.** A fifth check beside `no-ceiling`,
   `error-findings`, `expired-relaxation` and `already-expired` (ADR-0010): `subjects-mismatch`,
   the signed `subjects.groups` differ from the effective policy's groups, sorted. It is the member
   that decides who may claim, so code other than `evaluate` confirms it before anything is signed.
   `notaio_fail_open_total` gains the value.

8. **Conformance.** The vectors gain v4 cases every consumer's verifier must agree with: a v4 bundle
   accepted; a v3 payload under the v4 type and the reverse (`Schema`); a v3 payload with a v4 member
   (`Schema`); a v4 payload without `subjects` (`Malformed`); a v4 payload with an optional member
   written `null` (`DigestMismatch`); a v4 payload with a member v4 does not have, digested with it
   (`Malformed`). The v3 vectors stay until N+2. `bundle-v3.schema.json` is
   frozen and `bundle-v4.schema.json` is generated beside it, with `subjects` required.

9. **Three releases, each with notes written for consumers.** N: both schemas verify, the producer
   emits v3. N+1: the producer emits v4 by default. N+2: v3 is gone from the verifier, the types,
   the vectors and the setting, and `subjects` is required in the type. Each release opens with a
   hand-written "For consumers" section, and the install guide gains an "Upgrading to bundle v4"
   box. The notes of N+1 say that the cutover moves every policy's version once, which a consumer
   that pins running claims to a version must plan for (mediatore's ADR-0061 `follow`).

## Security analysis

| Threat | STRIDE | Control |
| --- | --- | --- |
| A v3 bundle read as v4, its missing `subjects` taken as "anyone" | E | Decision 4: a v3 payload cannot carry v4 members; `schema_version()` says which it is; `VerifyOptions` refuses v3 where subjects are enforced; v4 without `subjects` is `Malformed`; `[]` means nobody |
| A replayed v3 bundle after the cutover | T | Decision 6 and the floor (ADR-0008): the replay is an older version |
| A v4 bundle signed under the v3 type, or the reverse | T | Decision 3: the pair is checked, and the type travels with the payload through out-of-band signing |
| A member a reader does not model | T | Fails closed as `DigestMismatch`; decision 8's vectors pin it |
| A bug in `evaluate` signs the wrong groups | E | Decision 7, a check outside `evaluate`, counted |
| Emitting v4 before consumers can read it | D | Decision 5: an operator's switch, reversible, after consumers run N; decision 9's notes |
| An author sets `from` to disguise a grant's origin | S | Decision 2: `from` is not in the CRD; only the controller writes it, from `compose` |

## Consequences

- One migration for four ADRs. Eligibility by group (ADR-0024, mediatore ADR-0066) and the origin of
  a composite's grants (ADR-0023) become signed facts once N+1 is out; K2.4 and the source claim
  only need their producers filled, not a schema.
- Three releases and one consumer release between N and N+1. An installation that turns `v4` on
  before its consumers move refuses every bundle until it turns it off.
- Every policy re-signs once at the cutover.
- The bundle crate grows three wrapper types and two constants per schema for two releases.
- `docs/architecture.md`'s bundle row is corrected with the first of these releases; it still says
  v2.
- **CALM step deferred:** notaio has no CALM model yet; `docs/architecture.md` carries the change.
  The threat model pass is owed once N+1 ships.

## Alternatives rejected

- **A frozen v3 type and an enum of the two.** Precise, but every caller breaks twice: to add v4 and
  to drop v3.
- **`from` on the shared types.** It would appear in the CRD, settable by an author, and would break
  every consumer test that builds those structs.
- **The release is the switch.** A rollback would be a controller downgrade.
- **One schema per field ADR.** Four consumer migrations for what is one change to one document.
- **Wait for the model's K2.4 numbers.** ADR-0019's shape is fixed and its numbers are data; waiting
  would hold eligibility, which is a security gap today, on a parameter that changes no type.
