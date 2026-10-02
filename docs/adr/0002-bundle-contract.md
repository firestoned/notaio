# ADR-0002: The bundle contract

- Status: Proposed
- Date: 2026-09-30

## Context

Notaio produces something the runtime must trust without trusting notaio's network
position. The guest must never evaluate policy for itself, and a stale or downgraded bundle must not
loosen a running sandbox.

## Decision

1. **Envelope.** A DSSE envelope, payload type
   `application/vnd.firestoned.sandboxpolicy.bundle.v1+json`, signed with Ed25519. Signatures cover the
   DSSE pre-authentication encoding, never the bare payload.
2. **Content.** Policy and profile references, the effective step of every register knob, sorted
   audiences, egress, tools, lease and budgets. Canonical form: fixed field order, ordered maps,
   sorted lists, no floats. The digest is SHA-256 over those bytes.
3. **Version.** Monotonic, and it bumps only when the content digest changes. `issuedAt` and
   `notAfter` refresh without a version bump.
4. **Validity.** `notAfter` is at most one bundle lifetime away and never later than the earliest
   active grant expiry. A relaxation therefore ends on its own.
5. **Verification.** A consumer checks the payload type, a signature from a trusted key, the schema,
   that the content digest matches the content, that `issuedAt` is not in the future beyond a small
   skew, that `notAfter` has not passed, and that the version is not below the highest it has already
   accepted. Any failure is a refusal.

## Open decisions

1. **Distribution.** v0 publishes the envelope to a ConfigMap next to the policy. That makes every
   consumer a Kubernetes API reader, which sits badly with "mediatore never watches CRDs" and "the
   guest never talks to a Kubernetes API server". Alternatives: an OCI artifact in a registry,
   consumed by mediatore over its existing trust path, or a push from notaio to mediatore.
   The format above is independent of the choice.
2. **Signing key custody.** T8.2 says the signing key must not be held by cluster administrators.
   The development signer reads a seed from a Secret, which they can read. The production signer must
   be a `Signer` implementation backed by a KMS, HSM or TPM, with the public key distributed to
   consumers out of band.
3. **Tightening latency.** With no revocation channel, a tightened or deleted policy takes up to one
   bundle lifetime to stop being honoured by a consumer that does not re-fetch. Options: a shorter
   lifetime, a version floor pushed to mediatore, or both.
4. **Canonicalisation.** The current form is deterministic by construction. RFC 8785 would make it
   verifiable by non-Rust consumers. Decide before the schema is frozen.
5. **Cedar.** If conditional rules appear, the bundle carries a compiled Cedar policy set next to the
   declarative content. The consumer stays mediatore. The envelope does not change.

## Consequences

Consumers need one small crate and a set of trusted public keys. Notaio is the only writer.
Anything that can read the bundle can verify it, and anything that cannot verify it gets nothing.
