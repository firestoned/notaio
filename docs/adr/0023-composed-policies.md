# ADR-0023: A policy composed of others, signed as one bundle

- **Status:** Accepted
- **Date:** 2026-11-24
- **Deciders:** Erick Bourgeois
- **Related:** Roadmap 04 (the plan and its phases); ADR-0002 and ADR-0013 (the bundle); ADR-0007
  (the ExposureCeiling and Security's approval); ADR-0016 and ADR-0019 (bundle schema v4, which
  decision 7 joins); ADR-0018 (tightening latency); ADR-0020 (`SandboxAudience`); ADR-0021
  (status written only by notaio). In mediatore: ADR-0026 (one `policyRef` per sandbox), ADR-0061
  (a running claim follows a new bundle), and ADR-0065 (rejected in favour of this). Threat
  model T5.4, T8.2, K5.3, objective O3

Implemented (2026-11-25) but for decision 7's bundle v4 fields: the composition in
`sandboxpolicy-core`, `notaio lint`, the controller (the watch on includes, `status.includes` and
`status.includedBy`), the `e2e_compose` suite, and the threat model pass. The `includes` and
`from` payload fields land with bundle schema v4 (ADR-0026, roadmap 05); until then the includes are
recorded in status only, as decision 7 says.

## Context

A sandbox names exactly one policy (mediatore ADR-0026), and the consumer reads exactly one
signed bundle for it. A workload that needs the grants of several policies has no way to say so.
The case that opened this is Claude Code cloning a private repository, installing its Python
dependencies and pulling its own release: five audiences (`anthropic`, `github-git`, `pypi`,
`claude-downloads`, `mirror`) under `claude-agent`, whose bundle grants one. The only answer
today is a third policy that copies the grants of the others. The copy drifts from its sources,
and nobody reviews it as a combination.

mediatore ADR-0065 proposed composing at the sandbox (`spec.policyRefs`, combined by the broker
at admission). It is rejected for three reasons, given in full in roadmap 04 section 2:

- **Nobody approves the union.** Each policy passes its own ceiling, but the union is bounded by
  no ceiling and seen by no review.
- **Sandbox-wide parts have no natural combination.** Lease, knobs, budgets and subjects would
  each need an order or a minimum rule, and each such rule is a place to get the order wrong.
- **One signed artifact stops being the answer to "what was this claim allowed".** The claim
  record, the follow check, the shadow verdict and the audit line would all carry a list of
  bundles and a digest the consumer computed.

Composing in notaio keeps all three. The composite is a policy: it has a ceiling, it is linted,
and it is signed. No consumer changes to enforce it.

## Decision

1. **Where the composition is named.** `SandboxPolicy.spec.includes` is an optional list of
   `{ name }` references to other `SandboxPolicy` objects **in the same namespace**, one to four
   items, names unique, checked by a CEL rule in the CRD. The including policy (the
   *composite*) is an ordinary policy: a sandbox's `policyRef` names it, it needs its own
   `ExposureCeiling`, and it is published as its own bundle. The field is additive in
   `v1alpha2`; no new API version. A new kind (`SandboxPolicySet`) was considered and rejected: a
   fifth published kind and a second shape of bundle for every consumer.

2. **What an include contributes: named parts, nothing else.** An include contributes its
   `audiences` (keyed by `name`), `egress` (keyed by `endpoint`) and `tools` (keyed by `name`).
   Everything sandbox-wide is the composite's own and is written there: `profileRef`, `subjects`,
   `lease`, `budgets`, the knobs, the KnobGrants that relax them, and `resources` once ADR-0019
   lands. A KnobGrant for an include relaxes the include's own knobs and never reaches a
   composite.
   - An entry the composite declares itself **replaces** an included entry with the same key,
     whole.
   - Two includes that supply the same key, which the composite does not declare itself, are
     refused (**L031**). The composite resolves the collision by declaring the entry.
   - Never a field-by-field merge: a merged grant is one nobody wrote.
   - The order of `includes` means nothing. Composition is a function of the set, so reordering
     cannot change a grant, a digest or a version.

3. **One level.** An included policy may not itself have `includes` (**L032**). A policy that
   names itself is the same case, since it then has includes. L032 also covers the shape the CRD
   enforces, for `notaio lint`, which has no API server: more than four includes, a name twice,
   or `includes` on a policy with `publish: false`. No cycles by construction, and a
   reader sees every grant of a composite one step away. Nesting is revisited only if real
   policies need it.

4. **The composite is judged on its effective content, alone.** The effective content is the
   composite's own spec with its includes' named parts added under decision 2. All of today's
   policy lint runs on it, against the composite's profile and the composite's knobs after its
   own grants: L005 to L010, L017 to L020, and with audiences on, L027 and L028 resolved from the
   composite's namespace (the same namespace as every include). The ceiling checks, L014 to L016,
   compare the effective exposure against the **composite's own** `ExposureCeiling`, so Security
   approves the combination as a combination. An include's own conditions are not an input: an
   include matters by existing, by having no includes (L032), and by its subjects (decision 5).
   An entry that is valid under the include's profile but not under the composite's is refused in
   the composite with the code it would get there, and the finding names the include.

5. **Eligibility does not widen through an include.** Every group in the composite's
   `subjects.groups` must be in every include's `subjects.groups` (**L033**). The author of an
   include scoped its grants to some groups, and a composite must not hand them to others. An
   include with no groups can therefore be included by no composite, which is the conservative
   reading of a policy that binds nobody.

6. **A failing or missing include stops re-signing; nothing is loosened.** A missing include
   (`Ready=False`, reason `IncludeNotFound`, naming it), an include with includes (L032), a
   collision (L031), a subject mismatch (L033), or effective content that fails lint or its
   ceiling all leave the composite with no new bundle. Its last bundle is served until it expires
   (ADR-0018), as for any policy that fails lint today. A change to an include that would take a
   composite past its ceiling is the same case: L014 on the composite, not a new bundle.

7. **What the bundle and the status say about includes.** In bundle schema v4, with ADR-0016's
   `sources` and ADR-0019's `resources` (one migration, not three), the payload carries
   `includes: [{ name, uid, contentDigest }]`, sorted by name, and each named entry carries
   `from: <include name>`, absent for the composite's own. `contentDigest` is the `sha256:` digest
   of the RFC 8785 form of what the include contributes (its sorted `audiences`, `egress` and
   `tools`), not of its whole bundle, so a change to an include's lease or knobs, which no
   composite takes, does not churn every composite. **Consumers treat `includes` and `from` as
   audit data only:** the signature covers the effective content, and that is what they enforce.
   A consumer does not resolve or fetch an include, and never refuses a bundle because it cannot
   see one. Until v4 ships, the payload stays v3 with the effective content, and the includes are
   recorded only in status: `status.includes` (`name`, `uid`, `contentDigest`) on the composite,
   and `status.includedBy` (names, sorted) on each include. Both are written only by notaio
   (ADR-0021). A change to an include that changes the composite's effective content is a new
   composite version through the existing rule (the version moves when the digest moves).

8. **A policy that exists only to be included: `spec.publish: false`.** It defaults to `true`.
   A policy with `publish: false` is linted against its own profile as today (L001 to L013, L017
   to L020, L027, L028), but the ceiling checks (L014 to L016) do not apply, no bundle is
   compiled or signed, no ConfigMap is written, and it reports `Ready=True` with reason
   `NotPublished`. A ConfigMap left from an earlier `publish: true` is not deleted (notaio has no
   `delete` on ConfigMaps, and this does not add it): it is no longer refreshed, so its bundle
   expires within the bundle lifetime (ADR-0018), as for any policy that stops publishing. A sandbox that names it is refused by the consumer as `NotFound`, because no
   bundle exists. `publish: false` together with `includes` is refused by the CEL rule: such a
   policy could be neither run nor included. A published policy may also be included; `publish`
   says only whether the policy is a bundle of its own.

9. **Where it lives.** The composition is pure and lives in `sandboxpolicy-core`
   (`compose(policy, includes) -> EffectivePolicy`, with the origin of every named entry), so the
   controller and `notaio lint` run the same code. `notaio lint` resolves includes across the
   files it is given; a missing include is an input error (`I012`). The controller indexes
   includes by name and requeues every composite when an include changes, is deleted or
   re-signs.

## Consequences

- A sandbox that needs several policies' grants names one policy and reads one bundle. Each grant
  is written once, in the policy that owns it, and one Security-approved ceiling bounds the whole.
- **An include's owner changes every composite.** A widening in an include reaches every
  composite at its next evaluation. It is bounded by each composite's ceiling (decision 4) and
  visible in `status.includedBy` and, from v4, in each entry's `from`. The include's owner and the
  composite's reviewers may be different people, which is the point of the composite's ceiling.
- **Narrowing an include narrows every composite.** Removing an audience from an include removes
  it from every composite at its next evaluation, within the tightening latency of ADR-0018.
- **Re-signing fans out.** One include change re-evaluates every composite. The controller
  already re-signs at half life, so this adds work, not a new kind of work.
- **A composite's profile must allow what it includes.** A composite on a tighter profile than an
  include refuses that include's entries with the ordinary lint codes. This is deliberate: the
  composite's knobs are what the sandbox runs under.
- **New codes:** L031, L032, L033, I012; condition reasons `IncludeNotFound` and `NotPublished`.
  Each lint rule lands with the test that fails when it is weakened.
- **The CRD gains `spec.includes`, `spec.publish`, `status.includes` and `status.includedBy`.**
  Additive in `v1alpha2`; an object written without them behaves exactly as today.
- **No consumer change before v4.** Until then the origin of a grant is in the policy's status,
  not in the signed bundle, so mediatore's `PolicyConformant` and audit line cannot yet name the
  include a finding is about.
- **Rules out** composition at the consumer (mediatore ADR-0065), nested includes, cross-namespace
  includes, and any merge of two entries with the same key.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
