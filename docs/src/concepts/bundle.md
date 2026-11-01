# The bundle

The **bundle** is what notaio publishes for each Ready policy: the complete, effective policy,
signed. Every enforcement point (the gateway, mediatore, mediatore-guest inside the VM) verifies it
itself before acting on it. The contract is [ADR-0002](../adr/0002-bundle-contract.md); this page
is the overview.

## What is in it

A DSSE envelope (payload type `application/vnd.firestoned.sandboxpolicy.bundle.v3+json`) around a
JSON payload:

```json
{
  "schema": "sandbox.firestoned.io/bundle/v3",
  "meta": { "version": 4, "issuedAt": "…", "notAfter": "…" },
  "content": {
    "policy":   { "namespace": "sandboxes", "name": "agent-default", "uid": "…" },
    "profile":  { "name": "hardened", "digest": "sha256:…" },
    "knobs":    { "K1.1": "R0", "K1.2": "R1", "…": "every knob, grants applied" },
    "audiences": [ { "name": "internal-git", "access": "Read", "scopes": [ … ], "ttlSeconds": 300,
                     "endpoints": ["git.foo.io:443"], "rules": [ { "method": "GET", "path": "/repos/**" } ] } ],
    "egress": [ { "endpoint": "host:port", "rules": [ … ] } ], "tools": [ … ],
    "lease":    { "maxDurationSeconds": 14400, "maxConcurrent": 1 },
    "budgets":  { "maxToolCalls": 500, "maxWallClockSeconds": 14400, "maxEgressBytes": 10485760 }
  },
  "contentDigest": "sha256:…"
}
```

- **`content`** is everything that decides what a sandbox may do. Every knob is present, with
  grants applied, so a consumer never has to know the profile or the grants.
- **`contentDigest`** is SHA-256 over the RFC 8785 canonical form of `content`
  ([ADR-0005](../adr/0005-canonical-form-rfc-8785.md)), so a consumer in any language can recompute
  it.
- **`meta.version`** moves only when the digest moves, and never goes backwards.
- **`meta.notAfter`** is `issuedAt` plus the bundle lifetime (an hour by default), clamped to the
  earliest expiry of any grant in it.

## How a consumer decides to trust it

`sandboxpolicy_bundle::verify_bundle` fails closed, in this order:

1. The payload type is exactly the one above.
2. At least one signature verifies (ed25519) against a key the consumer trusts, by key id.
3. The schema is exactly `sandbox.firestoned.io/bundle/v3`.
4. The recomputed content digest matches `contentDigest`.
5. `issuedAt` is not in the future, allowing 60 seconds of clock skew.
6. `notAfter` has not passed.
7. The version is not below the consumer's floor.

The floor is the highest version the consumer has accepted for that policy, persisted before the
bundle is acted on, so a restart cannot reopen a downgrade
([ADR-0008](../adr/0008-consumer-version-floor.md)). It is keyed by namespace and name, with the
policy's `uid` ordered by `issuedAt`, so a recreated policy is accepted and a replay from its old
incarnation is refused. `sandboxpolicy_bundle::floor` implements it.

## How a consumer proves it gets this right: the conformance vectors

Every consumer (mediatore, mediatore-guest, the gateway) implements the checks above in its own
code path: its own key configuration, its own floor store, its own relay. A test that says "it
verifies" proves little; what matters is that it **accepts exactly what notaio accepts and refuses
exactly what notaio refuses**.

The conformance vectors are that test, as data. Each vector is a real, signed envelope plus
everything needed to judge it: the consumer's clock, the floor it holds before, and the verdict it
must reach, with the floor it must be left holding. Fourteen of them cover first sight, a newer
version, a recreated policy, and every way to be refused: a downgrade, a stale incarnation, a
forged signature, an untrusted key, expiry, a bundle from the future, a digest mismatch, an old
schema, an old payload type and a malformed payload. They are signed with a test-only key that is
published with them, so anyone can run them, and generated from fixed seeds and clocks, so they
never change unless the contract does.

notaio runs every vector through its own verifier in CI. A consumer runs the same file through its
own, in its CI. If both pass, they agree. The list is in
[Conformance Vectors](../reference/conformance.md), and how to run them is in
[Consume a Bundle](../guides/consume-a-bundle.md#test-against-the-conformance-vectors).

## Why the transport does not matter

The bundle is published as a ConfigMap, `<policy>-bundle`, read by mediatore through the API and
relayed unchanged to guests ([ADR-0003](../adr/0003-bundle-distribution.md)). Anyone who can write
that ConfigMap can withhold a bundle or replay an older one, and nothing more: a forged bundle
fails the signature, an older one fails the floor, and a stale one expires. Withholding is a denial
of service, bounded by the bundle lifetime: past `notAfter` the consumer refuses the bundle it
holds. What a consumer does while it has no valid bundle is its own decision (ROADMAP M3); it can
never be to run looser than the last bundle it verified.

## Who signs

In production, a non-exportable ed25519 key in OpenBao transit
([ADR-0004](../adr/0004-signing-key-custody.md)). notaio authenticates with its ServiceAccount and
may only request signatures; it never holds the key, and OpenBao audits every request. The key id
in the envelope names the key version, `notaio:v<N>`, so a rotation is visible to consumers.

## When notaio signs

A policy is signed only when every step below passes, in this order. A failure at any step signs
nothing new: the policy's `Ready` condition gives the reason
([every reason, and what to do](../guides/troubleshooting.md#by-ready-reason)), and the bundle
already published stays in place, valid until its `notAfter`.

1. **Only the leader signs.** A replica without the Lease does nothing, and the leader checks it
   still holds the Lease again just before it asks for a signature
   ([ADR-0011](../adr/0011-leader-election.md)).
2. **The evaluator compiles a bundle.** The profile exists and passes lint; grants that expired or
   lack an approval are left out; the policy passes every lint rule against the effective knobs;
   the exposure, active grants included, is within an approved ExposureCeiling; the content has a
   canonical form. The version goes up only when the content's digest changes, and `notAfter` is
   one bundle lifetime away, or the earliest active grant's expiry if that is sooner.
3. **Something changed.** An unchanged policy is not signed again until its bundle is within half
   its lifetime of `notAfter`, so with the default hour a policy nobody touches is re-signed about
   every 30 minutes, at the same version.
4. **The fail-open guard agrees.** A second, independent check of what the evaluator already
   guarantees: a ceiling exists, no lint finding is an error, no applied grant has expired, and
   `notAfter` is in the future. It never refuses in a correct build; `notaio_fail_open_total`
   counts it if it does ([ADR-0010](../adr/0010-health-metrics-and-events.md)).
5. **OpenBao signs.** A refusal, an unreachable OpenBao or a sealed one is `SigningFailed`, a
   Warning Event and `notaio_signing_failures_total`; the policy's status keeps the previous
   version and digest, which are what consumers still hold.

Only then is the `<policy>-bundle` ConfigMap written and `Ready=True, Published` set.
[The life of a change](life-of-a-change.md) follows the same steps from the pull request onward.

## What the signature does not cover

The bundle signature proves that notaio computed this effective policy from the objects it found in
the cluster, and when. It says nothing about how those objects got there. That is the job of the
steps before notaio: review in the policy repository, `notaio lint` in its CI, a GitOps reconciler
as the only identity allowed to change them, and, if your GitOps tool supports it, a signature on
what it delivers, checked before it applies anything.

The two cannot replace each other. A signature over the delivered policy is checked once, when it
is pulled, and covers what someone wrote; the bundle is computed in the cluster from the policy, its
profile, the grants active at that moment and its ceiling, re-signed as grants expire and its
lifetime runs out, and checked by every consumer, which does not trust the Kubernetes API it reads
the bundle from. Signing what is delivered is planned in the project's roadmap 03.
