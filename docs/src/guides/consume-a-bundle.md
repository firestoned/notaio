# Consume a bundle

For authors of the components that enforce policy: mediatore, mediatore-guest and the egress
gateway. A consumer never evaluates policy; it verifies the bundle and enforces what it says
([ADR-0002](../adr/0002-bundle-contract.md)). `sandboxpolicy-bundle` is the only crate you need,
and it has no Kubernetes dependency.

## Depend on the crate

Pin a release tag ([ADR-0014](../adr/0014-consumer-distribution.md)):

```toml
[dependencies]
sandboxpolicy-bundle = { git = "https://github.com/firestoned/notaio", tag = "v0.3.0" }
```

and allow the source in your `deny.toml`:

```toml
[sources]
allow-git = ["https://github.com/firestoned/notaio"]
```

The crate has no Kubernetes, async, networking or TLS dependency, and notaio's CI keeps it that
way (`make bundle-deps-check`). Move the tag deliberately, with the release notes: a new bundle
schema is called out there.

## Where the bundle is

A ConfigMap `<policy>-bundle` in the policy's namespace, key `bundle.dsse.json`
([ADR-0003](../adr/0003-bundle-distribution.md)). mediatore reads it through the API server and
relays it unchanged to the guest. A guest, which has no route to any API server, gets it only that
way. Where you read it from does not affect whether you trust it.

## Which keys to trust

The public keys of the signer, by key id `notaio:v<N>`, from
[the signer setup](openbao-signer.md#4-give-consumers-the-public-key). Configure them; never read
them from the same place as the bundle.

```rust
let mut keys = TrustedKeys::new();
keys.insert("notaio:v1", public_key_bytes)?;     // [u8; 32]
```

During a [key rotation](rotate-the-signing-key.md) you trust two versions for a while.

## Verify, record, then act

Keep the highest version accepted per policy across restarts, and write it down before acting on
a bundle ([ADR-0008](../adr/0008-consumer-version-floor.md)):

```rust
use sandboxpolicy_bundle::floor::{accept, FileFloor};

let mut floor = FileFloor::open(Path::new("/var/lib/consumer/floor.json"))?;
match accept(&envelope, &keys, Utc::now(), &mut floor) {
    Ok(payload) => enforce(payload),          // verified, and the floor is already on disk
    Err(refusal) => keep_last_good(refusal),  // forged, expired, a downgrade, or a stale replay
}
```

`accept` runs every check of [`verify_bundle`](../concepts/bundle.md#how-a-consumer-decides-to-trust-it),
then compares with the floor:

| Refusal | Cause |
| --- | --- |
| `NoValidSignature` | No signature verifies against a trusted key |
| `DigestMismatch`, `Schema`, `PayloadType`, `Malformed` | The payload is not what notaio produces |
| `NotYetValid`, `Expired` | Outside its validity window (60 seconds of skew allowed) |
| `Downgrade` | A lower version than the floor, for the same incarnation of the policy |
| `StaleIncarnation` | A bundle from an earlier incarnation of a recreated policy |
| `Floor` | The floor could not be read or stored. The bundle is refused, never accepted unrecorded. |

`FileFloor` writes atomically (a temporary file, synced, renamed), and a damaged file is an error,
never an empty floor. Implement `VersionFloor` for another store if a file does not suit you.

If your store is async, or the floor lives in a database row you read and write in one
transaction, call the decision on its own and keep the order: verify, decide, store, then act.

```rust
use sandboxpolicy_bundle::floor::next_floor;

let payload = verify_bundle(&envelope, &keys, &VerifyOptions::at(Utc::now()))?;
let held = store.floor(&policy.namespace, &policy.name).await?;    // in your transaction
let next = next_floor(held.as_ref(), &payload)?;                   // Downgrade, StaleIncarnation
store.set_floor(&policy.namespace, &policy.name, &next).await?;    // commit, then
enforce(payload);
```

## Try it

```sh
kubectl -n team-docs get configmap docs-agent-bundle \
  -o jsonpath='{.data.bundle\.dsse\.json}' > bundle.dsse.json
cargo run -p sandboxpolicy-bundle --example verify_and_record -- \
    bundle.dsse.json notaio:v1 <public-key-base64> floor.json
# accepted: team-docs/docs-agent version 1 (uid ...), valid until ...
```

Run it twice with an older bundle to see the floor refuse it. Exit codes: 0 accepted, 1 refused,
2 bad input.

## Test against the conformance vectors

notaio publishes signed bundles with the verdict every consumer must reach: accepted (first sight,
the same version, a newer one, a new incarnation) and refused (a downgrade, a stale incarnation, a
forged signature, an untrusted key, expired, not yet valid, a digest mismatch, an old schema, an old
payload type, a malformed payload). They are listed in [Conformance Vectors](../reference/conformance.md)
and come with the crate:

```rust
use sandboxpolicy_bundle::conformance::{suite, Expect};

let s = suite()?;
let keys = s.keys()?;                       // the vectors' own, test-only key
for v in &s.vectors {
    // Your verifier and your floor, seeded with v.floor for s.policy, at clock v.now.
    let got = your_verify_and_record(&v.envelope, &keys, v.now, v.floor.clone());
    match (&v.expect, got) {
        (Expect::Accepted { version, floor_after }, Ok(accepted)) => { /* compare both */ }
        (Expect::Refused { error }, Err(e)) => { /* compare with error_name(&e) */ }
        (want, got) => panic!("{}: expected {want:?}, got {got:?}", v.name),
    }
}
```

A consumer in another language reads `crates/sandboxpolicy-bundle/conformance/vectors.json` from
the tag it pins, and the payload's JSON Schema beside it, `bundle-v3.schema.json`. Run them in
your CI, so a change to your relay, your key configuration or your floor store that would accept
what notaio refuses fails there first.

## What to enforce

Everything in `payload.content`: every knob's step (all of them are present, grants applied), the
audiences with their access and token lifetime, the egress allowlist, the pinned tools, the lease
and the budgets. The schema is checked exactly, so a bundle with a different layout is refused
before your code sees it.
