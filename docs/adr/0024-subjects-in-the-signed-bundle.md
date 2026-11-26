# ADR-0024: The policy's subjects in the signed bundle, in schema v4

- **Status:** Accepted
- **Date:** 2026-11-25
- **Proposed:** 2026-11-25
- **Deciders:** Erick Bourgeois
- **Amended:** 2026-11-26 (accepted with [ADR-0026](0026-bundle-schema-v4-and-its-rollout.md), which carries the schema half and the rollout; decision 6's vectors path corrected)
- **Related:** [ADR-0002](0002-bundle-contract.md) (the bundle contract);
  [ADR-0005](0005-canonical-form-rfc-8785.md) (canonical form);
  [ADR-0008](0008-consumer-version-floor.md) (the consumer's version floor);
  [ADR-0013](0013-bundle-v3-routes-and-v1alpha2.md) decision 7 (a schema change is a new
  version a consumer refuses rather than misreads); [ADR-0016](0016-source-in-the-bundle.md) and
  [ADR-0019](0019-resource-sizes.md) (the planned bundle v4, which this joins);
  [ADR-0018](0018-tightening-latency.md); [ADR-0023](0023-composed-policies.md) (a composite's
  subjects are its own, bounded by `L033`); threat model K7.2; mediatore ADR-0054 decision 7
  (eligibility by subject waits on this change)

Partly implemented (2026-11-26): `L034` (decision 3), in the controller and `notaio lint`. The
bundle member (decisions 1, 2, 4 to 6) is roadmap 05's, under ADR-0026.

## Context

A `SandboxPolicy` carries `spec.subjects.groups`: the identity provider groups whose members may
claim a sandbox under it. notaio validates the list (`L010` warns when it is empty; `L033` bounds a
composite's list by each include's, ADR-0023) and then drops it: `BundleContent` holds the policy,
profile, knobs, audiences, egress, tools, lease and budgets, and no subjects. So the one document
every consumer trusts, the signed bundle, cannot answer "who may use this policy". ROADMAPS.md still
lists eligibility by `subjects.groups` (K7.2) as open.

A consumer that wants to answer it has two choices today, both worse than the bundle:

- Read `spec.subjects` from the `SandboxPolicy` object in the cluster. That is unsigned state:
  whoever can write the object changes who may claim, and the argument of mediatore ADR-0026
  decision 2 against taking rules from the cluster applies word for word.
- Keep a second list of policies and groups in the consumer's own configuration. That is the
  two-documents problem the signed bundle exists to end.

Two things on main shape the answer:

- **A v4 bundle is already planned.** ADR-0016 (`sources`) and ADR-0019 (`resources`) share one
  `bundle/v4`, and ADR-0023 adds `includes` and `from` to it, so that consumers migrate once. This
  ADR adds one more field to that same migration, not a fourth schema.
- **Composition is in notaio** (ADR-0023). A composite's `subjects` are its own and are signed as
  the effective content; `L033` already refuses a composite whose groups are wider than any include's.
  So the list a consumer reads from a bundle is already the final one, with no composition for the
  consumer to do.

Group matching is, and stays, exact: a group is an opaque string whose spelling is the identity
provider connector's. notaio does not interpret, normalise or pattern-match it.

## Decision

1. **`content.subjects`, a new member of the bundle content:** `{ "groups": [ ... ] }`, the policy's
   groups verbatim (for a composite, its own, ADR-0023 decision 2). The list has set semantics: it is
   sorted (byte order) and a duplicate is a lint error (decision 3), so the canonical form
   (ADR-0005) is stable however the author ordered it. Unlike `resources` and `sources`, `subjects`
   is not optional in v4: an empty list is written `[]`, and a consumer reads `[]` as "nobody" (fail
   closed, as `L010` already says). A v4 bundle without the member is refused as malformed.

2. **It is part of the one `bundle/v4`.** `SCHEMA` is `sandbox.firestoned.io/bundle/v4` and the payload
   type `application/vnd.firestoned.sandboxpolicy.bundle.v4+json`, as ADR-0016, ADR-0019 and ADR-0023
   plan. Whichever of those is built first builds the schema half once, with every field in the types
   and the conformance vectors, `subjects` included. Because DSSE signs the payload type, a v3 bundle
   can never verify as v4 nor the reverse, and a v3 consumer refuses a v4 bundle rather than
   misreading it (ADR-0013 decision 7). The audience bundle (ADR-0020, ADR-0022) is a different type
   and is unchanged.

3. **New stable code `L034` (error): a malformed group entry.** An entry is refused when it is empty,
   has leading or trailing whitespace, contains a character from `*?[]{}` (a pattern the matcher
   would read as a literal, which an author never means), or repeats another entry of the same
   policy. It applies in the controller and in `notaio lint`. This only adds findings: no existing
   rule, ceiling or knob maximum is loosened.

4. **The version stays monotonic across the schema change.** `meta.version` continues from the policy's
   last published version, so moving to v4 bumps it like any digest change and a consumer's floor
   (ADR-0008) refuses a replayed v3 bundle of the same policy as an older version. notaio never
   publishes a v3 and a v4 bundle for one policy side by side.

5. **A change of groups is a content change, and a narrowing is a tightening.** It moves the digest
   and the version, and reaches consumers on the schedule of ADR-0018. A composite's groups follow
   the include rule: narrowing an include's groups below a composite's turns `L033` on the composite
   and stops its re-signing until the author fixes it (ADR-0023 decision 6). Widening a policy's list
   widens who may use what the ceiling already approved; it does not widen the exposure, so
   `ExposureCeiling` is unchanged (see open question 1).

6. **Upgrade order: consumers first,** as ADR-0016 decision 5 has it. A consumer accepts v3 and v4
   before notaio emits v4, because it refuses unknown schemas. The conformance vectors
   (`crates/sandboxpolicy-bundle/conformance/vectors.json`) gain v4 cases every consumer's verifier must agree with, including a
   v3 bundle refused as v4 and the reverse, and a v4 bundle with no `subjects` refused.

7. **No behaviour changes in notaio's decisions.** notaio does not decide eligibility; it only signs
   the list. `notaio lint` applies `L034` at pull request time with the same code as the controller.

## Security analysis

| Threat | STRIDE | Control |
| --- | --- | --- |
| A consumer reads who may claim from unsigned cluster state | T/E | The list is in the signed content; the consumer needs nothing else |
| A v3 bundle (no subjects) replayed to a consumer that now checks subjects | T | Distinct payload type (decision 2), and the version floor (decision 4) |
| A consumer treats a missing list as open | E | v4 always carries the list; `[]` means nobody; a v4 bundle without it is refused; a v3 bundle is refused by a v4-only reader |
| A composite carries groups wider than an include allows | E | `L033` (ADR-0023), applied before signing; the signed list is the composite's own |
| An author writes a pattern believing it is matched | S/E | `L034` refuses pattern characters; matching stays exact |
| Group spelling differs by connector | S | Not normalised here: the author lists each spelling the consumer should accept |
| A policy author widens who may use an approved exposure | E | Out of the ceiling's scope today: open question 1; review and CODEOWNERS on the policy directory |

## Consequences

- Consumers can enforce "who may use this policy" from the signed bundle alone, for composites too.
- Adding a group, or removing one, is a new bundle version, with the usual validity window.
- An upgrade needs consumers first, then notaio; an older consumer behind a newer notaio sees
  refusals, not misreads. No second schema migration: the field rides on the planned v4.
- The CRD is unchanged (`spec.subjects.groups` already exists); only the bundle and lint change.
- **CALM step deferred**, as for every ADR until the model is bootstrapped:
  `docs/architecture.md` carries this change. The threat model pass is owed before this is
  marked implemented.

## Alternatives rejected

- **Consumers read the SandboxPolicy object.** Unsigned and cluster-writable (Context).
- **A separate signed "subjects" bundle.** Two documents that can disagree and two floors to
  keep; the content of one policy is one signed unit.
- **A `bundle/v5` for this field alone.** A second consumer migration where one is planned.
- **Patterns or captures in `groups`.** Makes a group spelling a parameter of a signed grant and
  needs a registry of valid values to be safe; exact matching with a generated list gives the same
  reach without a new class of mistake.
- **Normalising connector prefixes in notaio.** It would make two groups of different
  organisations the same group (mediatore ADR-0046 decision 2 rejects the same idea).

## Open questions

1. **Should a ceiling bound the groups?** Today Security approves what is reachable and the
   policy author decides who may reach it; within a composite `L033` bounds the author by the
   includes. A ceiling that lists the groups a policy may name would put "who" under Security
   too. *Recommendation:* not in this ADR; decide it with ADR-0025, where it matters because
   several policies share one ceiling.
2. **Should `L010` become an error?** An empty list publishes a bundle nobody can use. Kept a
   warning to avoid breaking existing repositories.
