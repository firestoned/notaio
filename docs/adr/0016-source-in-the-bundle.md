# ADR-0016: The source of a policy, recorded in its bundle

- **Status:** Proposed
- **Date:** 2026-11-06
- **Deciders:** Erick Bourgeois
- **Amended:** 2026-11-08 (decision 5: one v4 with ADR-0019)
- **Related:** ADR-0015 (signed policy delivery), ADR-0002 and ADR-0013 (the bundle), ADR-0008
  (the consumer's floor); roadmap 03 ("later, separate decision"); roadmap 00 D6 (what a token
  carries)

Proposed, not built: nothing needs it yet. It is written down so the design is settled before a
consumer asks for it, and so the schema change it implies is planned rather than discovered.

## Context

A bundle says what notaio computed and when, signed. It does not say where the policy came from.
With signed delivery (ADR-0015) that question has a good answer upstream: the policy arrived in an
artifact CI built from one commit of the policy repository, and the ceiling in one Security signed.
A consumer, or an auditor reading a token minted under the bundle (roadmap 00, D6), cannot follow
that chain from the bundle alone: it has to correlate timestamps with GitOps logs.

Two facts constrain the design:

- **A new field in the bundle is a breaking change.** `verify_bundle` recomputes the content digest
  from the parsed content, so a consumer whose types do not know a field drops it and refuses the
  bundle (`DigestMismatch`). Any addition is a new schema, and consumers must accept it before
  notaio emits it.
- **notaio does not depend on a GitOps tool.** It reads the policy objects and nothing about how
  they arrived; the install guide works with Flux, Argo CD or anything else that is the admitted
  identity.

## Decision

1. **The source is a claim made by the delivery pipeline, carried on the objects.** An optional
   annotation, `sandbox.firestoned.io/source`, on a `SandboxPolicy`, its `ExposureCeiling` and any
   `KnobGrant`: a short string the pipeline chooses, recommended as `<repository>@<commit>` (for
   ADR-0015's CI, the policy repository and the commit it built from; the artifact digest follows
   from the artifact's own `revision` annotation). CI stamps it when it renders the artifacts.
2. **notaio copies it and does not verify it.** It cannot: the claim's integrity comes from the path
   (GitOps-only admission lets only the reconciler set it; signed delivery proves who built or
   approved the object). notaio puts it, verbatim, into the bundle, where the bundle's signature
   then covers it.
3. **In the bundle, as `content.sources`:** `{ policy, ceiling, grants: { <name>: <source> } }`,
   each present only when its object carried the annotation. A policy with no annotations gets no
   `sources`; the rest of the bundle is unchanged.
4. **No GitOps API.** notaio does not read Flux's `Kustomization` or `OCIRepository` to discover a
   revision: that would tie the controller to one tool and widen its RBAC for metadata a pipeline
   can attach itself.
5. **A schema bump, consumers first.** The bundle becomes `bundle/v4`, shared with ADR-0019
   (Amendment). The crate learns to verify both v3 and v4 in one release; consumers move to it;
   only then does notaio emit v4, and the conformance vectors gain v4 cases. A consumer on the old crate meanwhile keeps receiving v3.

## Consequences

- A consumer or auditor can name, from a verified bundle, the commit its policy and ceiling were
  built from, and from there the signed artifacts that carried them.
- The source is only as trustworthy as the pipeline that stamps it. Without signed delivery it is
  whatever the policy repository's `main` said; with it, it is what CI or Security signed. The
  bundle does not make a weak pipeline strong, and says so by copying rather than verifying.
- A schema migration with two releases, and the consumers' cooperation, for a field that is purely
  informational to them. That cost is why this waits for a consumer that needs it: mediatore
  recording the policy's source in the tokens it mints (D6), or an audit that asks for it.
- **CALM step deferred:** notaio has no CALM model yet.

## Amendment (2026-11-08): one schema change with ADR-0019

This ADR and ADR-0019 (resource sizes for K2.4) each change the bundle to `bundle/v4`, each with a
consumers-first rollout of two notaio releases. Done separately, that is two migrations, and the
second is `v5`. They become one:

- **v4 carries both fields, both optional:** `content.resources` (ADR-0019, decision 4) and `content.sources` (ADR-0016, decision 3). Absent means "not stated": no K2.4
  statement, or no source claim. A consumer treats an absent `resources` as K2.4 unchecked, as
  today.
- **The schema half is built once, by whichever ADR is built first,** with both fields in the
  types and the conformance vectors: `sandboxpolicy-bundle` verifies v3 and v4, consumers take
  it, then notaio emits v4. The other ADR then only starts filling its field: no schema change,
  no second migration.
- **The field shapes are fixed now,** so the schema half can be built ahead of either ADR's own
  trigger: `resources` is `{ vcpus, memoryMiB, diskGiB }` (unsigned integers), `sources` as in
  decision 3 here. The sizes awaiting the model are data in `Ceilings`, not schema, so a
  change to them later changes no type.

## Alternatives considered

- **Read the revision from Flux's objects.** Precise (Flux knows the artifact digest), but it binds
  notaio to Flux and needs read access to its kinds. Rejected (decision 4).
- **Record the artifact digest instead of a commit.** The digest is not known until the artifact is
  pushed, so CI cannot stamp it into the content it pushes. The commit identifies the content as
  well, and the artifact names its commit.
- **Add the field to v3.** Every consumer on the current crate would refuse every bundle the moment
  notaio emitted it. Rejected (decision 5).
