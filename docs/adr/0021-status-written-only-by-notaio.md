# ADR-0021: Status is written only by notaio

- **Status:** Accepted
- **Date:** 2026-11-22
- **Deciders:** Erick Bourgeois
- **Related:** Extends ADR-0007 decision 6 (ceilings change only through GitOps); ADR-0003 (the
  published ConfigMap as a second source of the version); ADR-0020 decision 6 (collisions decided
  by what is published); threat model T8.2, scenario S6

## Context

`deploy/admission/gitops-only.yaml` refuses every create, update and delete of the policy kinds
except from the GitOps reconciler. Its rules name the main resources (`sandboxpolicies`, …). A
ValidatingAdmissionPolicy rule matches a subresource only when the subresource is named, so the
`/status` subresources of `SandboxProfile`, `SandboxPolicy`, `KnobGrant` and `SandboxAudience`
were outside it. That was deliberate for one writer, notaio, whose status writes had to pass. It
also let through every other identity RBAC allows to patch status.

notaio does not treat status as a report only. It reads it back as input:

- `status.bundleVersion` and `status.bundleDigest` of a policy or an audience are the previous
  version and digest the next one is computed from (`reconcile.rs`, `audience.rs`), together
  with the published ConfigMap's annotations;
- `status.bundleNotAfter` decides when a bundle is refreshed;
- `status.bundleDigest` of an audience decides its standing in a collision (L024, L025): an
  audience whose published digest matches its content is stable and wins against one that changed.

Whoever can patch status can therefore make notaio publish below the version consumers hold,
which they refuse (an outage), delay a refresh until the bundle expires, report a false `Ready`,
or forge an audience's standing so that a platform audience's next change is refused in every
namespace. None of it loosens a signed bundle, because the content still comes from the spec. It
is still an edit to the policy system's state that no review saw, which is what T8.2 is about.

Status cannot simply be ignored on input: the version must never go backwards even if one of the
two sources is lost (ADR-0003), and the collision rule needs to know what is published.

## Decision

1. **A second ValidatingAdmissionPolicy, `sandbox-status-notaio-only`, in the same manifest**
   (`deploy/admission/gitops-only.yaml`). It matches `UPDATE` of `sandboxprofiles/status`,
   `sandboxpolicies/status`, `knobgrants/status` and `sandboxaudiences/status` (a patch is an
   update to admission) and admits only notaio's ServiceAccount,
   `system:serviceaccount:notaio-system:notaio`. `failurePolicy: Fail`.
2. **The GitOps reconciler does not write status,** so it is not admitted there. A spec is
   reviewed; a status is what notaio concluded from it.
3. **One manifest, applied and removed together.** Installing the GitOps-only rule installs this
   one; the install guide, `make kind-e2e-admission` and `cluster-undeploy` need no new step.
4. **An installation that runs notaio under another namespace or ServiceAccount edits the
   username,** as it edits the GitOps identity today.
5. **Break glass is the same as for the spec rule:** a cluster administrator can delete the
   binding. That is outside what admission can stop (`SECURITY.md`).

## Consequences

- Only notaio can change what notaio reads back from status, so the version, the refresh and the
  collision standing come from notaio's own earlier decisions or from the ConfigMap it publishes.
- The ConfigMap remains a second, weaker source: whoever can write ConfigMaps in a policy
  namespace can still raise the version annotation (bounded, ADR-0003 and `next_version`) or
  delete the bundle (an outage until the next refresh). This ADR does not change that.
- A cluster administrator can no longer hand-edit status, for example to clear a condition while
  debugging. Deleting the binding is the way, and it is visible.
- `e2e_admission` proves both halves: notaio still publishes with the policy applied, and another
  identity with RBAC to patch status is refused.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
