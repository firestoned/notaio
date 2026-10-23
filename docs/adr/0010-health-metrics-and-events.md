# ADR-0010: Health, metrics and Events

- **Status:** Accepted
- **Date:** 2026-10-23
- **Deciders:** Erick Bourgeois
- **Related:** ROADMAP M2 ("health endpoints, Prometheus metrics, Kubernetes Events"); the
  threat model's metric "relaxations past their expiry: 0" and its relaxation dashboard;
  ADR-0004 (the signer, whose failures these report); ADR-0007 (exposure ceilings)

## Context

notaio's only outputs are object status and the bundle ConfigMaps. An operator cannot tell from
outside whether the controller is up, whether a signer outage is holding back every bundle, or
whether a relaxation outlived its expiry. The threat model asks for a metric, "relaxations past
their expiry: 0", and a relaxation dashboard. ROADMAP M2 lists health endpoints, Prometheus
metrics and Kubernetes Events.

Two standing rules are in the way:

- notaio "takes no inbound connection" (`docs/architecture.md`, `CLAUDE.md`). Kubelet HTTP probes
  and a Prometheus scrape are both inbound.
- its RBAC is "read plus status writes". An Event is a new object, so recording one is a write.

The rule against inbound connections exists because notaio holds a signing identity. Nothing that
reaches it may influence what it signs. The rule on RBAC exists because the controller must never
be able to change policy. Neither reason is touched by a listener that ignores its input, or by a
write that cannot reach a policy object.

## Decision

1. **One listener, narrow by construction.** Plain HTTP on port 8080 (`NOTAIO_HTTP_ADDR`
   overrides the address), serving exactly three paths: `GET /healthz`, `GET /readyz` and
   `GET /metrics`. Any other method or path gets 404 or 405. The request body is never read,
   and no path takes a parameter. The handler reads the controller's state and changes nothing. A
   bounded number of connections, and a header-read timeout. No TLS: the responses carry no
   secret, and the NetworkPolicy limits who can ask.
2. **Who can reach it.** The NetworkPolicy admits TCP 8080 only, from a namespace labelled
   `kubernetes.io/metadata.name: monitoring` (edit the label to your scraper's namespace). Probes
   come from the kubelet on the pod's own node. Whether a NetworkPolicy applies to that traffic
   depends on the network plugin; the e2e suites prove it on kind and k0s, and a plugin that blocks
   it needs an `ipBlock` rule for the node addresses.
3. **Health.** `/healthz` answers 200 while the process serves. `/readyz` answers 200 once the
   listener's caches of profiles, policies and grants have their first full list, and 503 before. The Deployment
   uses them as liveness and readiness probes.
4. **Metrics.** Gauges are computed at scrape time from caches the listener keeps of the three
   kinds, so a deleted object leaves no stale series. Counters count since start. The caches are
   the listener's own, not the controllers' stores: a kube-rs store's readiness wakes one waiter,
   and the controller already waits on its own, so a second waiter there never woke.
   - `notaio_policies{reason}`: policies by their `Ready` reason (`Published`, `SigningFailed`,
     and so on).
   - `notaio_lint_failures{kind}`: objects failing lint now (`SandboxProfile` and `SandboxPolicy`
     with `Valid` false, `KnobGrant` rejected).
   - `notaio_relaxations{phase}`: knob grants by phase.
   - `notaio_relaxation_age_seconds{namespace,grant}` and
     `notaio_relaxation_expires_in_seconds{namespace,grant}`: each active relaxation, for the
     dashboard.
   - `notaio_relaxations_past_expiry`: active grants whose `expiresAt` has passed. **Must be 0.**
     It is non-zero only between an expiry and the reconcile it schedules, so alert on it held for
     a few minutes.
   - `notaio_bundle_age_seconds{namespace,policy}` and
     `notaio_bundle_expires_in_seconds{namespace,policy}`: the published bundle.
   - `notaio_bundles_published_total`, `notaio_signing_failures_total`,
     `notaio_reconcile_errors_total`.
   - `notaio_fail_open_total{check}`: **must be 0.** Before publishing, the controller checks the
     bundle independently of the evaluator: a ceiling exists, the evaluation found no error, every
     relaxation in it is unexpired, and it is not already past `notAfter`. If any check fails,
     nothing is published, the policy reports `Ready=False` with reason `FailOpenRefused`, and
     this counter counts it. A non-zero value means the evaluator has a bug.
   Labels carry namespace and object names only for per-object gauges, never message text.
5. **Events.** notaio records `events.k8s.io/v1` Events on transitions only, never on every
   reconcile: a new bundle version (`Published`), a `Ready` reason change to a failure
   (`SigningFailed`, `FailOpenRefused`, or the evaluator's reason), a grant becoming `Expired` or
   `Rejected`, and a profile becoming invalid. A failure to record an Event is logged and never
   fails a reconcile.
6. **RBAC.** The ClusterRole gains `create` and `patch` on `events.k8s.io` `events`, which is what
   the event recorder needs, and nothing else: no `update`, `delete`, `get` or `list`, and no
   core-group events. Policy objects stay read plus status.

## Consequences

- The no-inbound rule is now "no inbound connection that can influence signing". The listener
  reads nothing from its peer but a request line, and serves only what it reads from its own
  caches. `docs/architecture.md`, the NetworkPolicy and `CLAUDE.md` say this.
- Policy, grant and namespace names are visible to whatever can scrape. The NetworkPolicy keeps
  that to the monitoring namespace.
- Anyone who can read Events in a policy namespace sees notaio's Events there. They repeat what
  the object's status already says.
- `e2e_rbac` asserts the new verbs and that nothing broader came with them. `e2e_install` scrapes
  the deployed controller through a port-forward and checks that both must-be-zero series are 0.
- One dependency, `prometheus-client` (Apache-2.0 OR MIT, from the Prometheus project).
- Three more watches on the API server (profiles, policies, grants), for the caches.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
