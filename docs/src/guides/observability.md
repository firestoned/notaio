# Observe notaio

notaio serves `/healthz`, `/readyz` and `/metrics` on port 8080 and records Kubernetes Events on
transitions ([ADR-0010](../adr/0010-health-metrics-and-events.md)). Nothing else listens.

## Scraping

The NetworkPolicy admits port 8080 only from the namespace named in its ingress rule, `monitoring`
by default; change it to wherever Prometheus runs. There is no Service: scrape the pod directly.
With the Prometheus Operator:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PodMonitor
metadata:
  name: notaio
  namespace: notaio-system
spec:
  selector:
    matchLabels:
      app.kubernetes.io/name: notaio
  podMetricsEndpoints:
    - port: http
      path: /metrics
```

With plain Prometheus, a `kubernetes_sd_configs` job of role `pod` in namespace `notaio-system`,
keeping pods labelled `app.kubernetes.io/name=notaio` and port name `http`, does the same.

## The metrics

| Metric | Type | Meaning |
| --- | --- | --- |
| `notaio_policies{reason}` | gauge | Policies by their `Ready` reason |
| `notaio_lint_failures{kind}` | gauge | Profiles and policies with `Valid` false, and rejected grants |
| `notaio_relaxations{phase}` | gauge | Knob grants by phase |
| `notaio_relaxation_age_seconds{namespace,grant}` | gauge | Age of each active relaxation |
| `notaio_relaxation_expires_in_seconds{namespace,grant}` | gauge | Time until each active relaxation expires |
| `notaio_relaxations_past_expiry` | gauge | Active grants past `expiresAt`. **Must be 0.** |
| `notaio_bundle_age_seconds{namespace,policy}` | gauge | Age of each published bundle |
| `notaio_bundle_expires_in_seconds{namespace,policy}` | gauge | Time until each published bundle passes `notAfter` |
| `notaio_bundles_published_total` | counter | Bundles signed and published |
| `notaio_signing_failures_total` | counter | Signing attempts that failed |
| `notaio_reconcile_errors_total` | counter | Reconciles that failed and were retried |
| `notaio_fail_open_total{check}` | counter | Bundles refused by the fail-open guard. **Must be 0.** |

Gauges are computed at scrape time, so a deleted object leaves no stale series. Counters reset when
the pod restarts. Both must-be-zero series exist at 0 from the start, so an alert compares a value,
not an absence.

## Alerts to start from

```yaml
groups:
  - name: notaio
    rules:
      - alert: NotaioFailOpenRefused
        expr: increase(notaio_fail_open_total[1h]) > 0
        labels: { severity: critical }
        annotations:
          summary: "notaio refused to sign a bundle the evaluator produced ({{ $labels.check }}): a bug"
      - alert: NotaioRelaxationPastExpiry
        expr: notaio_relaxations_past_expiry > 0
        for: 5m
        labels: { severity: critical }
        annotations:
          summary: "An active relaxation is past its expiry"
      - alert: NotaioSigningFailing
        expr: increase(notaio_signing_failures_total[15m]) > 0
        for: 10m
        labels: { severity: warning }
        annotations:
          summary: "notaio cannot sign: is OpenBao sealed or unreachable?"
      - alert: NotaioBundleNearExpiry
        expr: notaio_bundle_expires_in_seconds < 600
        for: 5m
        labels: { severity: warning }
        annotations:
          summary: "Bundle for {{ $labels.namespace }}/{{ $labels.policy }} expires in under 10 minutes"
      - alert: NotaioPoliciesNotPublished
        expr: sum(notaio_policies{reason!="Published"}) > 0
        for: 30m
        labels: { severity: info }
      - alert: NotaioDown
        expr: absent(up{job=~".*notaio.*"} == 1)
        for: 5m
        labels: { severity: warning }
```

`NotaioRelaxationPastExpiry` waits five minutes because the gauge is briefly non-zero between an
expiry and the reconcile it schedules. A bundle normally refreshes at half its lifetime, so
`NotaioBundleNearExpiry` firing means refreshes are failing.

For a relaxation dashboard, table `notaio_relaxation_expires_in_seconds` by namespace and grant,
next to `notaio_relaxation_age_seconds`.

## Events

```sh
kubectl get events -A --field-selector reportingComponent=notaio
kubectl -n team-docs describe sandboxpolicy docs-agent    # Events at the bottom
```

| Object | Reason | Type | When |
| --- | --- | --- | --- |
| SandboxPolicy | `Published` | Normal | a new bundle version is published |
| SandboxPolicy | the `Ready` reason | Warning | `Ready` turns false, or its reason changes |
| KnobGrant | `Expired` | Normal | the grant lapses |
| KnobGrant | `Rejected` | Warning | the grant breaks a rule |
| SandboxProfile | `Invalid` | Warning | the profile turns invalid, or its findings change |

A steady state records nothing, so Events are a log of changes, not a heartbeat. Events for
cluster-scoped profiles are in the `default` namespace.

## Health

`/healthz` is 200 while the process serves, and is the liveness probe. `/readyz` is 503 until the
controller has its first full list of profiles, policies and grants, then 200; it is the readiness
probe. Neither says anything about OpenBao: a sealed OpenBao shows as `SigningFailed`, not as an
unready pod, because restarting notaio would not help.
