# ADR-0011: Leader election and replicas

- **Status:** Accepted
- **Date:** 2026-10-26
- **Deciders:** Erick Bourgeois
- **Related:** ROADMAP M2 ("leader election and more than one replica"); ADR-0002 (the bundle
  contract and its monotonic version); ADR-0008 (the consumer's version floor); ADR-0010 (health
  and metrics)

## Context

notaio runs as one replica. When its pod is rescheduled, no bundle is signed or refreshed until a
new one starts, and a bundle that is not refreshed expires at `notAfter`, after which consumers
refuse it. One replica is therefore a bound on availability that operators cannot raise.

Two replicas that both reconcile would both sign and both publish. Their writes to the bundle
ConfigMap and to status would race, and the controller's assumption that one writer moves the
version forward would no longer hold. kube-rs has no leader election of its own.

## Decision

1. **One leader, by a Kubernetes Lease.** A `coordination.k8s.io/v1` Lease named `notaio` in the
   controller's namespace. Every replica keeps the health and metrics listener and its caches
   running, so every replica is ready and a rollout completes. Only the replica holding the Lease
   runs the reconcilers, signs and publishes.
2. **Our own loop, no new dependency.** About 150 lines on the Lease API notaio already uses
   through `kube`. Whether to create, renew, take over or wait is a pure function of the Lease, the
   replica's identity and `now`, unit tested; writes use the Lease's `resourceVersion`, so two
   replicas cannot both win a take-over.
3. **Timings.** A Lease lasts 15 seconds. The leader renews every 5 seconds, and gives up if it
   has not renewed for 10, before the Lease can expire. A follower checks every 2 seconds.
4. **Losing the Lease ends the process.** A leader that cannot renew in time, or finds another
   holder, exits; the pod restarts as a follower with fresh state. Reconcilers also check
   leadership before every signature and status write, which narrows the window further. On a
   normal shutdown the leader releases the Lease, so a follower takes over at once.
5. **Identity.** The pod name, from the downward API (`NOTAIO_POD_NAME`). The controller refuses to
   start without it, because two replicas with the same identity would both lead. The Lease's
   namespace is the pod's own (`NOTAIO_NAMESPACE`, default `notaio-system`).
6. **RBAC.** A Role in the controller's namespace only: `get` and `update` on the Lease named
   `notaio`, and `create` on Leases there (Kubernetes cannot restrict `create` by name). Nothing
   cluster wide changes.
7. **Deployment.** Two replicas, a PodDisruptionBudget keeping one available, and a preferred
   anti-affinity that spreads them across nodes. A `notaio_leader` gauge is 1 on the leader and 0
   elsewhere.

## Why an overlap cannot loosen a sandbox

Leases bound how long two replicas can both believe they lead, but cannot rule it out: the
follower judges expiry by its own clock against the leader's renew time, so clock skew between
nodes can open a short overlap. During one, a stale leader can at worst publish a bundle compiled
from an older view. Its version is not higher than the newest one consumers have accepted, so the
version floor refuses it (ADR-0008). An overlap can therefore delay a change or withhold a bundle
for a moment; it cannot make any consumer act on a looser policy. The signing key and its audit
log are unchanged: both replicas use the same ServiceAccount and the same transit key.

## Consequences

- A pod loss no longer stops signing: a follower takes over within the Lease's 15 seconds, or at
  once on a clean shutdown.
- The controller gains one write outside status, Events and bundles: one Lease in its own
  namespace. `e2e_rbac` checks it is that and nothing more.
- A new e2e suite, `e2e_leader`, checks that exactly one replica leads, that the other takes over
  when the leader's pod is deleted, and that the new leader publishes.
- `NOTAIO_POD_NAME` becomes required. Running the controller outside a cluster needs it set.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
