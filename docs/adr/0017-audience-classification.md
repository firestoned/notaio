# ADR-0017: Audience classification, so Lab policies reach lab audiences only

- **Status:** Proposed
- **Date:** 2026-11-07
- **Deciders:** Erick Bourgeois
- **Related:** ADR-0007 (the ExposureCeiling and Security's approval); ADR-0015 (signed delivery:
  Security alone signs the ceilings); ADR-0013 (audiences in v1alpha2); threat model K5.2, K5.3,
  section 9 ("No toxic combinations"); `ROADMAPS.md` M4

Proposed: the design is here for review before any code. Nothing below is built.

## Context

The threat model's Lab profile relaxes K5.2 (token scope) and K5.3 (audience set) further than any
other profile, and says those steps "apply to lab audiences only". notaio cannot check that today:
an audience is a name, and nothing records which names are lab ones. `docs/threat-model-mapping.md`
lists it under "Not covered", and the AIR mapping cites it as the gap behind AIR-PREV-006.

The gap is wider than two knobs. The toxic-triangle rule (`L003`, `lint::triangle`) treats a
`LabOnly` profile as holding no sensitive data "by construction", so it never fires there. Lab is
also the profile with the most untrusted content (K1.1 at `R3`) and the widest outbound channel
(K4.1 at `R3`). A Lab policy that names an audience serving internal data therefore has all three
legs of the triangle, and the one rule written to refuse that combination is switched off for it.
The construction the rule relies on is exactly what is missing.

Three facts shape where a classification can live:

- **Who may say an audience is lab.** Calling an audience lab loosens what a policy may do with it,
  so the policy's author must not be the one to say it. ADR-0007 already has an object only
  Security approves (`L016`), and with ADR-0015 only Security can sign it into the cluster.
- **What notaio already compares.** `lint_exposure` checks every audience a policy exposes against
  its ExposureCeiling, by name, before any bundle is signed (`L014`). A per-audience fact on the
  ceiling is read at the point where it is needed, with no new watch.
- **The bundle does not need it.** Consumers enforce the audiences in the bundle; whether an
  audience is lab only decides whether notaio may sign the bundle at all. Keeping it out of the
  bundle avoids a schema change (contrast ADR-0016).

## Decision

1. **The classification is on the ceiling's audience.** `ExposureCeiling.spec.audiences[]` gains an
   optional `dataScope`, with the values profiles already use: `LabOnly` or `Internal`. It is an
   additive, optional field in `v1alpha2`; no API version changes.
2. **Unclassified means `Internal`.** An audience without `dataScope` is treated as serving internal
   data. Failing closed is the point: a lab classification is a claim Security makes on purpose.
3. **A new lint rule, `L020`, an error:** a policy whose profile is `LabOnly` exposes an audience
   whose ceiling entry is not `LabOnly`. It runs in `lint_exposure` beside `L014`, so a policy that
   fails it publishes no bundle, like any other ceiling failure, and `notaio lint` reports it on the
   pull request.
4. **Every audience of a Lab policy, not only the ones past Standard's steps.** The model's sentence
   could be read as limiting only the extra audiences and write scopes Lab allows. notaio reads it
   strictly, because `L003` depends on it: a Lab policy with even one internal audience has the
   whole triangle, and Standard's limits on it would not change that.
5. **Policies on `Internal` profiles are unaffected.** They may expose lab and internal audiences
   alike; the classification only narrows `LabOnly`.
6. **Visible on a dashboard.** The Grafana dashboard notaio ships (`deploy/monitoring/`) shows
   policies by `Ready` reason, so a Lab policy held back by `L020` shows up as not published, with
   the finding in its `Valid` condition.

## Consequences

- K5.2 and K5.3 "for Lab" move from "Not covered" to "Implemented here" in the threat model
  mapping, and `L003`'s assumption about `LabOnly` becomes something lint checks rather than
  assumes.
- **An upgrade can stop a Lab policy publishing.** A Lab policy whose ceiling does not classify its
  audiences fails `L020` on the first reconcile after the upgrade; its last bundle stays until it
  expires, and then its sandboxes can mint nothing. The release notes say to classify the ceilings
  of every `LabOnly` policy, and have Security sign them, before upgrading. `notaio lint` on the
  policy repository finds every case first.
- The classification is per ceiling, so two ceilings can classify the same audience differently.
  Both are Security's signatures, which is the same trust the rest of the ceiling already carries;
  notaio does not reconcile them. A lint warning across the ceilings `notaio lint` sees in one
  repository is a possible follow-up.
- Egress hosts and tools are not classified. K4.1 on Lab is bounded by the ceiling's
  `egressHosts`, which Security approves host by host; classifying them is not part of this
  decision.
- **CALM step deferred:** notaio has no CALM model yet; `docs/architecture.md` drops this from its
  known gaps.

## Alternatives considered

- **A cluster-scoped audience registry** (one object per audience, carrying its classification).
  One fact per audience, so no two ceilings can disagree; but a new kind, a new watch and new RBAC,
  admission and signed-delivery coverage, for the same signer (Security). Rejected for now; the
  field on the ceiling can move into a registry later if disagreements turn up.
- **On the policy or the profile.** The author would classify the audiences their own policy uses.
  Rejected (Context, first fact).
- **A warning first, an error a release later.** A gentler upgrade, but the warning release is the
  window in which a Lab policy can hold the whole triangle while lint says it is fine. Rejected;
  `notaio lint` finds every case before an upgrade.
- **Classify in the bundle, for consumers.** Nothing a consumer does depends on it. Rejected
  (Context, third fact).

## Open question for review

Decision 4 reads the model more strictly than its words. If the model means only the audiences and
write scopes beyond Standard's steps, the model or this ADR should change to say so, and `L003`
needs a different guard for Lab. This is the one point where the code would encode an
interpretation, so it is called out rather than settled quietly.
