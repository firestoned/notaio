# ADR-0019: Resource sizes for K2.4

- **Status:** Proposed
- **Date:** 2026-11-08
- **Deciders:** Erick Bourgeois
- **Amended:** 2026-11-08 (decisions 3 and 4: lint code `L022`, `resources` optional in the
  schema; one v4 with ADR-0016)
- **Related:** ADR-0002 (the bundle contract; this is a `SCHEMA` change), ADR-0013 (the last schema
  change, and how consumers took it), ADR-0008 (the consumer's floor); threat model K2.4, section
  10 (profiles); `ROADMAPS.md` M4

Proposed, not built. The numbers below are a proposal: the AgentSandbox model is the source of
truth for K2.4 and does not define the medium and large sizes yet. This ADR is accepted, and
built, only once the model states the sizes, and with the model's numbers if they differ.

## Context

K2.4, resource limits, is in the knob register (`registry.rs`) with three steps, owned by the
Platform Owner, and every built-in profile sets it: Fortress `R0`, Hardened and Standard `R1`, Lab
`R2`. Nothing reads it. `docs/architecture.md` says so ("K2.4 is not checked") and
`ceilings.rs` says why: the model names small, medium and large sizes without saying what medium
and large are.

So a policy cannot ask for a size, notaio cannot refuse one too large for its profile, and a
consumer has no signed statement of what size a sandbox may have. The steps are words.

Three things are already settled by how the other numeric knobs work:

- **K7.3 and K4.3 are the pattern.** Their steps map to numbers in `Ceilings` (lease length,
  concurrent sandboxes, egress bytes), a policy may set a value at or below the number for its
  effective step, an omitted value defaults to that number, and lint `L009` refuses one above it.
  The values are "starting points to tune, not commitments", so they are site data with a default,
  not constants.
- **The bundle carries what consumers enforce.** Lease and budgets are in `BundleContent`; a
  consumer enforces them from the verified bundle, not from the cluster.
- **The size is enforced where the VM is chosen.** banlieue gives a sandbox a VM from a pool
  (`VirtualMachineClaim.spec.poolRef`); the pool's template fixes its size, and the claim says
  nothing about size. Whatever makes the claim picks the size by picking the pool.

## Decision

1. **Three sizes, one per step.** Proposed, pending the model:

   | K2.4 step | Size | vCPUs | Memory | Disk |
   | --- | --- | --- | --- | --- |
   | `R0` | small | 1 | 2 GiB | 10 GiB |
   | `R1` | medium | 2 | 4 GiB | 20 GiB |
   | `R2` | large | 4 | 8 GiB | 40 GiB |

   They live in `Ceilings` beside K7.3's and K4.3's, as site data with these defaults.
2. **A policy may ask for less.** `SandboxPolicy.spec.resources`, optional, with `vcpus`,
   `memoryMiB` and `diskGiB`. Omitted, it is the size for the policy's effective K2.4 step, as an
   omitted lease is K7.3's. Integers only (ADR-0005: the canonical form has no floats).
3. **A new lint rule, `L022`, an error:** a requested value is zero or above the size for the
   effective K2.4 step. A separate code from `L009`, so a finding names the knob it comes from. A
   `KnobGrant` raising K2.4 raises the ceiling it is checked against, as for every knob.
4. **The bundle carries the size: schema v4.** `BundleContent` gains `resources`, optional in the
   schema (Amendment) and always set by notaio once this is built, defaulted as in 2. That is a change to the canonical form, so `SCHEMA` becomes
   `sandbox.firestoned.io/bundle/v4`, with the payload type to match, and ADR-0002 is amended.
5. **Consumers first.** A v3 consumer refuses a v4 bundle (ADR-0013, decision 7), so for one
   release `sandboxpolicy-bundle` verifies both v3 and v4 while notaio still publishes v3; consumers
   take that release; then notaio publishes v4 and the next release drops v3. The floor is
   unaffected: versions keep counting across the schema change.
6. **Enforcement is elsewhere.** Whatever claims the VM claims from a
   pool whose template is no larger than the bundle's `resources`, and refuses to start a sandbox
   when no pool fits. notaio does not read pools: it cannot reach banlieue's objects and should
   not (CLAUDE.md, "Do not"). The threat model mapping lists it under "Supported, enforced
   elsewhere".

## Consequences

- K2.4 becomes checked: a policy cannot ask for more than its profile allows without a grant the
  Platform Owner approves, and every consumer gets the size in signed form.
- **A schema change, with its rollout order** (decision 5): two notaio releases and one consumer
  release, in that order. An installation that upgrades notaio past the second without upgrading
  its consumers sees every bundle refused, so the release notes say so in both releases.
- Existing policies need no change: an omitted `resources` takes the step's size. A site whose
  pools are larger than the step's size for some profile finds its sandboxes refused by the
  claimer once enforcement lands, which is the point, and is said in that release's notes.
- Disk is a single number. Sandboxes with several disks, or with a scratch volume separate from
  the image, would need the model to say how K2.4 splits them.
- **CALM step deferred:** notaio has no CALM model yet; `docs/architecture.md` drops K2.4 from its
  known gaps when this is built.

## Amendment (2026-11-08): one schema change with ADR-0016

ADR-0016 (the source in the bundle) and this ADR each change the bundle to `bundle/v4`, each with a
consumers-first rollout of two notaio releases. Done separately, that is two migrations, and the
second is `v5`. They become one:

- **v4 carries both fields, both optional:** `content.resources` (this ADR, decision 4) and
  `content.sources` (ADR-0016, decision 3). Absent means "not stated": no K2.4 statement, or no
  source claim. A consumer treats an absent `resources` as K2.4 unchecked, as
  today.
- **The schema half is built once, by whichever ADR is built first,** with both fields in the
  types and the conformance vectors: `sandboxpolicy-bundle` verifies v3 and v4, consumers take
  it, then notaio emits v4. The other ADR then only starts filling its field: no schema change,
  no second migration.
- **The field shapes are fixed now,** so the schema half can be built ahead of either ADR's own
  trigger: `resources` is `{ vcpus, memoryMiB, diskGiB }` (unsigned integers), `sources` as in
  ADR-0016, decision 3. The sizes awaiting the model are data in `Ceilings`, not schema, so a
  change to them later changes no type.

## Alternatives considered

- **Named sizes in the bundle (`size: medium`), numbers in banlieue's pools.** Shorter, but the
  meaning of "medium" would then live in pool templates nobody signs, and two clusters could
  disagree. Numbers in the bundle make the signed statement self-contained.
- **No policy field, the size is always the step's.** Simpler, but a policy that needs less than
  its profile allows could not say so, and K2.4 would be the only numeric knob without a request.
- **Fold it into `L009`.** One code for every numeric ceiling hides which knob a finding is about,
  and `L009`'s documented meaning names K7.3 and K4.3.
- **Wait for the model before writing anything.** The rollout order in decision 5 takes two
  releases; deciding the shape now lets the consumer half start when the numbers arrive, not
  after.
