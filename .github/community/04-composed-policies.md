<!-- Copyright (c) 2026 Erick Bourgeois, notaio -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap 04: a policy composed of others, signed as one bundle

- **Status:** In progress: phase 0 decided in ADR-0022, phases 1 to 3 done (2026-11-24); opened
  2026-11-24. Replaces mediatore roadmap 16 and its ADR-0065
  (several `policyRefs` per sandbox, composed by the broker), which are withdrawn in favour of this.
- **Milestone:** M4 in [`ROADMAPS.md`](../../ROADMAPS.md)
- **Related:** ADR-0002 and ADR-0013 (the bundle); ADR-0007 (the ExposureCeiling and Security's
  approval); ADR-0016 and ADR-0019 (bundle schema v4); ADR-0018 (tightening latency); ADR-0020
  (`SandboxAudience`, the shared definition of an audience). In mediatore: ADR-0026 (one
  `policyRef` per sandbox), ADR-0054 (which policies a namespace may name), ADR-0061 (a running
  claim follows a new bundle), and the withdrawn ADR-0065.

## 1. Goal

A workload that needs the grants of several policies runs under one policy that **includes**
them, and notaio signs the result as one bundle under one ceiling. Each grant is written once,
in the policy that owns it; the combination is reviewed once, as a whole, by Security; and every
consumer keeps reading exactly one verified bundle per sandbox.

The case that opened this: Claude Code cloning a private repository, installing its Python
dependencies and pulling its own release. Its sandbox names five audiences (`anthropic`,
`github-git`, `pypi`, `claude-downloads`, `mirror`), and `claude-agent`'s bundle grants only
`anthropic`. Under mediatore's shadow mode the run is allowed and marked `PolicyConformant=False`
(`Gaps`, four `P001`s); under enforce it would be refused. Today the only fix is a third policy
that copies the grants of the others (`claude-git-agent`, which already drifts and has no
ceiling).

## 2. Why here and not in the broker

mediatore ADR-0065 proposed the composition at the sandbox: `spec.policyRefs`, composed by the
broker at admission. It is withdrawn for three reasons:

- **Nobody approves the union.** Each policy passes its own ceiling, but what a sandbox may do
  becomes the union of several, which no ceiling bounds and no review sees. ADR-0065's worst case
  (direct egress from one policy beside an audience from another) was left to an operator switch.
- **Combining sandbox-wide parts has no natural meaning.** Lease, knobs, budgets and subjects need
  an order or a minimum rule, and each such rule is a place to get the order wrong.
- **One signed artifact stops being the answer to "what was this claim allowed".** The claim
  record, the follow check (ADR-0061), the shadow verdict and the audit line would all carry a
  list of bundles and a composite digest computed by the consumer.

Composing in notaio keeps all three intact: the composite is a policy, it has a ceiling, it is
linted, and it is signed. No consumer changes to enforce it.

## 3. Decisions

**D1. Where the composition is named.** *Recommended:* `SandboxPolicy.spec.includes`, an optional
list of `LocalObjectReference` (other `SandboxPolicy` objects in the same namespace), one to four
items, names unique. The including policy is an ordinary policy: it is named by a sandbox's
`policyRef`, needs its own `ExposureCeiling`, and is published as its own bundle.
*Alternative:* a new kind, `SandboxPolicySet`, that holds only references. Cleaner to read, but a
fifth published kind and a second shape of bundle for every consumer. *Rejected:* composition by
the consumer (mediatore ADR-0065, section 2).

**D2. What an include contributes.** *Recommended:* named parts only: `audiences` (by `name`),
`egress` (by `endpoint`) and `tools` (by `name`). Sandbox-wide parts (`profileRef`, `lease`,
`subjects`, knobs and budgets, `resources` once ADR-0019 lands) are the including policy's own and
must be written there. An entry the including policy declares itself replaces an included entry of
the same key, whole. Two includes that supply the same key are refused (`L031`) unless the
including policy declares that key itself. Never a field-by-field merge: a merged grant is one
nobody wrote. Order of `includes` means nothing, so reordering cannot change a grant.
*Alternative:* the first include wins on a collision (ADR-0065's rule); rejected, because it makes
a reordering a change of grant that lint cannot see.

**D3. One level.** *Recommended:* an included policy may not itself have `includes` (`L032`).
No cycles by construction, and a reader sees every grant of a composite one step away.
*Alternative:* nesting with cycle detection; revisit only if real policies need it.

**D4. The ceiling bounds the composite.** *Recommended:* `L014` to `L016` apply to the
composite's effective content, after includes are resolved, against the composite's own
`ExposureCeiling`. Security approves the combination as a combination. A change to an included
policy that would take a composite past its ceiling stops that composite's re-signing
(`Ready=False` with `L014`), and it keeps serving its last bundle until that bundle expires
(ADR-0018), as any policy that fails lint does today.

**D5. Eligibility does not widen through an include.** *Recommended:* every group in the
composite's `subjects.groups` must be in every included policy's `subjects.groups` (`L033`). The
author of an include scoped its grants to some groups; a composite must not hand them to others.
*Alternative:* the composite's subjects alone; rejected for that reason.

**D6. What the bundle says about its includes.** The verdict and the audit need to name where a
grant came from, and a consumer's floor must move when an include changes.
*Recommended:* the payload carries `includes: [{ name, uid, contentDigest }]` and each named entry
its origin (`from: <include name>`, absent for the composite's own), added in bundle schema v4
with ADR-0016's `sources` and ADR-0019's `resources`, so there is one migration. Any change to an
include is a new version of every composite that includes it. Until v4 ships, the controller
re-signs on an include's change and records the includes in the policy's status only.
*Decided (ADR-0022 decision 7):* audit data only; the signature covers the effective content.

**D7. A policy that exists only to be included.** Such a policy has no ceiling of its own and
today reports `Ready=False` with `L015`, which reads as an alarm. *Decided (ADR-0022 decision 8):* `spec.publish: false`: lint it, never publish it, report
`Ready=True` (`NotPublished`); a sandbox naming such a policy is refused by the consumer as
`NotFound`, because no bundle exists.

## 4. Work, in order

Each phase is shippable and leaves the system no looser than before.

### Phase 0: design

- [x] ADR-0022 (a policy composed of others), D1 to D7 decided (2026-11-24). D6: includes are
      audit data only. D7: `spec.publish: false`.
- [ ] mediatore ADR-0065 marked rejected, pointing here; mediatore roadmap 16 removed.

### Phase 1: the composition, pure (`sandboxpolicy-core`)

- [x] `spec.includes` and `spec.publish` in `sandboxpolicy-api` (v1alpha2), with the CEL rules
      for one to four unique names and for `publish: false` without `includes`; the CRD
      regenerated (`make manifests`).
- [x] `compose(policy, includes) -> EffectivePolicy`, with the origin of every named entry. Tests
      first: a union without collisions; the composite's own entry replacing an include's; two
      includes colliding refused (`L031`); an include with includes refused (`L032`); subjects
      wider than an include's refused (`L033`); a list of none equal to the policy itself.
- [x] `evaluate` and `L014` to `L016` run on the `EffectivePolicy`. `publish: false` is linted and
      reports `Ready=True` (`NotPublished`) with no bundle and no ceiling.

### Phase 2: `notaio lint`

- [x] Includes resolved across the files given; a missing include is an input error (`I012`). Text and SARIF name the include a finding is about.
- [x] `examples/`: a composite of two policies, with its ceiling (`sandboxpolicy-composite.yaml`,
      `exposureceiling-build-agent.yaml`).

### Phase 3: the controller

- [x] An index from include name to the policies that include it; a change, deletion or
      re-signing of an include requeues every composite (`include.rs`, a second watch on
      `SandboxPolicy` mapped through the controller's store).
- [x] A missing or failing include: the composite reports `Ready=False` with the include's name
      and keeps its last bundle until expiry.
- [x] Status: `includes` with each one's `uid` and content digest; on the include,
      `includedBy`. Both always written, so the status merge patch clears an emptied list.
- [x] e2e (`e2e_compose`): a composite published; an include widened past the composite's ceiling stops the
      re-sign; an include deleted; a reordering of `includes` produces the same bundle digest.

### Phase 4: the bundle (with schema v4)

- [ ] `includes` and per-entry `from` in the v4 payload (D6); conformance vectors for both; the
      JSON Schema regenerated.
- [ ] mediatore: `PolicyConformant` and the audit line name the include a finding is about (a
      display change; enforcement is unchanged).

### Phase 5: the homelab, docs and threat model

- [ ] In the policy repository: `pypi`, `claude-downloads` and `mirror` as policies of their own
      (`publish: false` under D7), and `claude-dev-agent` including `claude-agent`, `git-agent`
      and those three, with one ceiling approved by Security. `claude-git-agent` deleted. Five
      audiences need K5.3 at R2 on `claude-dev-agent`'s own profile (every built-in profile but
      `lab` allows one): a K5.3 KnobGrant on the composite, since an include's grants do not reach
      it.
- [ ] Live: the Claude Code workload of the opening section runs under `claude-dev-agent` with
      `PolicyConformant=True` in shadow, then refused nothing under `policy.audiences: enforce`.
- [ ] A guide page (composing policies) and the reference for `includes` and `L031` to `L033`.
- [ ] Threat-model pass: an include changed by its owner changes every composite; bounded by D4.

## 5. Done when

A sandbox that needs five audiences names one policy, that policy's bundle grants all five, each
grant is written in exactly one policy, and one ceiling approved by Security bounds the whole. No
consumer reads more than one bundle per sandbox.

## 6. Risks

- **An include's owner changes every composite.** A widening in `git-agent` reaches each policy
  that includes it at the next re-sign. Bounded by D4 (each composite's ceiling) and made visible
  by `includedBy` and the per-entry origin; the include's owner and the composite's reviewers may
  be different people.
- **Re-signing fans out.** One include change re-signs every composite. Small at today's scale;
  the controller already re-signs at half life, so this adds work, not a new kind of work.
- **Schema v4 is a coordinated migration.** D6 rides it rather than adding a fourth migration;
  until then the origin of a grant is in the policy's status, not in the signed bundle.
