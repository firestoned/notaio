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
