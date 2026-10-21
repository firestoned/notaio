# ADR-0008: The consumer's persisted version floor

- **Status:** Accepted
- **Date:** 2026-10-21
- **Deciders:** Erick Bourgeois
- **Related:** [ADR-0002](0002-bundle-contract.md) decision 5 (verification refuses a version below
  the highest accepted); threat model T5.4 (bundle downgrade); ROADMAP M1 ("example verifier for
  mediatore and mediatore-guest, with persisted highest version accepted")

Implemented with this ADR: `crates/sandboxpolicy-bundle/src/floor.rs`, the example in
`crates/sandboxpolicy-bundle/examples/verify_and_record.rs`, tests in
`crates/sandboxpolicy-bundle/tests/floor.rs`.

## Context

`verify_bundle` refuses a version below `min_version`, but the caller has to supply that number,
and it is only as good as the memory it comes from. A consumer that forgets its floor on restart
can be handed an older, looser bundle that is still signed and still inside its validity window:
exactly the downgrade T5.4 describes. Every consumer (mediatore, mediatore-guest, the gateway)
needs the same rule, so it belongs in the crate they share, not in each of them.

One detail decides the key. A policy that is deleted and recreated is a new object with a new
`uid`, and its versions restart at 1, because the old bundle ConfigMap is garbage collected with
the old policy. Keyed by namespace and name alone, the recreated policy's v1 would be refused
forever. Keyed by `uid` alone, a replayed bundle from the old incarnation, with its old `uid`,
would bypass the floor.

## Decision

1. **The floor is kept per policy namespace and name,** and records the `uid`, the highest
   version accepted and that bundle's `issuedAt`.
2. **Same `uid`: the version must not go down.** A lower version is refused (`Downgrade`); the
   same version again is a re-delivery and is accepted.
3. **A different `uid` is a new incarnation,** accepted only if it was issued after the bundle
   the floor holds, which then resets the floor to it. A bundle from an older incarnation is older
   by construction, so a replay is refused (`StaleIncarnation`). This relies on notaio's clock
   moving forward, which the `issuedAt` check against the consumer's clock already bounds.
4. **Verify, then persist, then accept.** `accept` verifies the envelope, checks the floor,
   writes the new floor durably, and only then returns the payload. If the floor cannot be
   written, the bundle is refused: a consumer never acts on a bundle it could forget.
5. **The crate provides it.** `sandboxpolicy-bundle` gains a `VersionFloor` trait, an in-memory
   implementation for tests and a file-backed one: one JSON file, written to a temporary file in
   the same directory, synced, then renamed over the old one, so a crash leaves either the old
   floor or the new one. A missing file is an empty floor; a file that does not parse is an
   error, never an empty floor. No new dependency, and still no Kubernetes dependency, so
   mediatore-guest can use it as is.
6. **A runnable example** shows the whole consumer loop: read an envelope, verify it against a
   trusted key, check and record the floor, report the result.

## Consequences

- Consumers link the crate rather than reimplementing persistence rules, which is where downgrade
  bugs come from.
- Recreating a policy needs no operator step: the new incarnation is accepted on its first bundle.
- A consumer whose floor file is lost (a wiped disk) starts empty and accepts the current bundle.
  That is the same position as a new consumer; the bundle's expiry still bounds what an attacker
  can replay. Mediatore-guest should keep its floor on the VM's own disk, which is per identity
  and never shared (platform ADR-0081).
- Deleting a policy leaves its floor entry behind. Entries are small; pruning them is left to the
  consumer.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
