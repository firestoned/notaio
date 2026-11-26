# ADR-0025: A ceiling shared by several policies, in a namespace of its own

- **Status:** Proposed
- **Date:** 2026-11-25
- **Deciders:** Erick Bourgeois
- **Related:** [ADR-0007](0007-exposure-ceiling.md) (the ceiling, one per policy);
  [ADR-0015](0015-signed-policy-delivery.md) (the ceiling stays the approvers' alone);
  [ADR-0017](0017-audience-classification.md); [ADR-0020](0020-sandbox-audience.md)
  (platform namespace, opt-in, decision 12); [ADR-0023](0023-composed-policies.md) (a composite
  has its own ceiling, and `L031` to `L033` are taken); [ADR-0024](0024-subjects-in-the-signed-bundle.md)

## Context

ADR-0007 fixes one ceiling per policy, "named after the policy it caps". The controller looks it
up by the policy's own name and namespace. That is the right default, and two real shapes
strain it:

- **Many generated policies of one family.** A platform that issues one policy per team from a
  template needs one approval pull request, but ends with one ceiling object per policy, each
  carrying the same approval reference and the same bounds.
- **A namespace whose owners define their own policies.** A team that is itself a platform for
  others (ADR-0020 decision 12 delegates delivery to the team's own repository) can write
  policies in its namespace, but every one needs a ceiling an approver has already written, so
  the team cannot add a policy without a ticket, even one strictly inside what was approved.

What must not change: the approvers alone decide the maximum, no policy publishes without a
covering ceiling, and nothing a team writes in its own namespace can cover itself.

## Decision

1. **Opt-in, like ADR-0020.** The **ceiling namespace** is named in the controller's
   environment (`NOTAIO_CEILING_NAMESPACE`) and in `notaio lint` (`--ceiling-namespace`). There is
   no default. While it is unset nothing in this ADR applies and behaviour is exactly ADR-0007's.

2. **`ExposureCeiling.spec.appliesTo`, optional and honoured only in the ceiling namespace:**

   ```yaml
   spec:
     appliesTo:
       namespaces: [team-a, team-b]     # exact names, non-empty
       policies: [reader, writer]       # exact names, non-empty, or `allPolicies: true`
   ```

   There are no patterns. Neither list may be empty, and `allPolicies: true` is spelled out, so
   "covers everything in these namespaces" is a deliberate line in a reviewed file, never an
   omission.

3. **Resolution.** A policy's own same-name ceiling in its own namespace, when present, is the only
   one consulted, exactly as today (no fallback, so adding a shared ceiling never changes a policy
   that has its own). Only when there is none, the controller looks in the ceiling namespace for
   ceilings whose `appliesTo` covers the policy.

4. **Covered by one ceiling, never by a union.** The policy's exposure must be inside a single
   covering ceiling. Two approved ceilings never add up to a third that nobody approved. Every
   audience, endpoint, egress host, tool, lease and classification rule of ADR-0007, ADR-0013 and
   ADR-0017 applies to the covering ceiling unchanged, and `L016` (an approval reference is
   required) applies to it too.

5. **A shared ceiling outside the ceiling namespace covers nothing.** An `appliesTo` on a ceiling
   in any other namespace is ignored and reported (`L035`, error, on the ceiling), so a team
   cannot write the object that covers its own policies. The status of a policy names the
   covering ceiling and its version, so approvers see what a policy was admitted under.

6. **No ceiling, no bundle, still.** A policy with no own ceiling and no covering one fails with
   `L015` as now, at pull request time and in the cluster. A finding on the covering ceiling
   blocks publication of a new bundle, and the last valid bundle stays until its lifetime ends
   (ADR-0018).

7. **Who writes the ceilings is a requirement on the deployment** (ADR-0015, ADR-0020 decision
   12): the ceiling namespace, and every `ExposureCeiling`, is writable only by the approvers'
   delivery identity, enforced by an admission policy like the one in `deploy/admission/`.
   notaio cannot check this; the install guide says so.

8. **New stable codes:**

   | Code | Meaning |
   | ---- | ------- |
   | L035 | `appliesTo` on a ceiling outside the ceiling namespace, or an empty or malformed `appliesTo` |
   | L036 | Two ceilings in the ceiling namespace name the same policy in the same namespace (a warning: either covers it, so the author sees the overlap) |

## Security analysis

| Threat | STRIDE | Control |
| --- | --- | --- |
| A team writes a ceiling that covers its own policies | E | Decision 5: honoured only in the ceiling namespace; decision 7: only the approvers write there |
| Two approved ceilings combine into more than either approved | E | Decision 4: one covering ceiling, never a union |
| A shared ceiling approves more than intended through a broad `appliesTo` | E | Exact lists only, `allPolicies` explicit, approval reference required (decisions 2, 4) |
| A shared ceiling changes a policy that already has its own | T | Decision 3: the own ceiling is the only one consulted |
| Unset configuration changes behaviour on upgrade | T | Decision 1: opt-in, no default |
| Blast radius: one object now approves many policies | E | Accepted and recorded: the approval pull request lists every covered policy; status names the covering ceiling |

## Consequences

- One approval can cover a family of generated policies, and a team that runs a platform can add
  policies inside an approved bound without a new ticket for each.
- A shared ceiling concentrates approval: widening it widens every policy it covers, so it is
  reviewed like the widest policy it covers.
- `status` gains the covering ceiling's name and version; the bundle and its schema are unchanged.
- **CALM step deferred.** The threat model pass is owed before this is marked implemented.

## Alternatives rejected

- **A ceiling in the team's own namespace.** The team would approve itself (ADR-0015).
- **Union of covering ceilings.** Combines approvals nobody saw (decision 4).
- **Name patterns in `appliesTo`.** One typo covers a family nobody meant to cover.
- **Keep one ceiling per generated policy.** Correct, and the fallback if this ADR is not
  accepted: the cost is object count, not safety.

## Open questions

1. **Should a shared ceiling also bound `subjects.groups`?** Without it, a team inside a covering
   ceiling can name any group, widening who may use what was approved (ADR-0024 open question 1).
   *Recommendation:* yes, as an optional exact list on the shared ceiling; when present a
   policy's groups must be a subset, and a new code covers it.
2. **Status of a policy whose covering ceiling disappears.** Under decision 6 it fails on the
   next reconcile and the last bundle expires; confirm that matches ADR-0018's tightening
   latency for a removed approval.
