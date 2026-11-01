# ADR-0004: The production signing key lives in OpenBao transit

- **Status:** Accepted
- **Date:** 2026-10-16
- **Deciders:** Erick Bourgeois
- **Resolves:** [ADR-0002](0002-bundle-contract.md) open decision 2 (signing key custody)
- **Related:** threat model T8.2 and scenario S6; mediatore's STS key, which uses the same
  OpenBao transit backend (mediatore threat model, asset A-1)

Implemented 2026-10-22: `crates/notaio/src/openbao.rs` (the transit client and the signer choice),
`crates/notaio/src/signer.rs`, `deploy/signer/` (the deployment patch and the egress rule), and the
`e2e_openbao` suite against a real OpenBao (`make kind-e2e-openbao`, `make openbao-e2e`). The
default since 2026-10-23: `deploy/controller/` signs with OpenBao, and every e2e suite runs that
deployment as written, on kind with cert-manager and dev/openbao. The development signer survives
only as the opt-in `deploy/signer/development-patch.yaml`, for local experiments; contrary to
decision 5's "and the e2e", no e2e suite uses it.

## Context

The threat model requires that the policy signing key is not held by whoever administers the
cluster (T8.2). The development signer violates that on purpose: anyone who can read the
`notaio-dev-signing-key` Secret can sign any bundle, forever, from anywhere.

The `Signer` trait (`crates/sandboxpolicy-bundle/src/dsse.rs`) was written so the key can live
outside the process: notaio needs a signature over the DSSE pre-authentication encoding, never the
key itself.

Options considered:

| Option | Key held by | Cluster admin can | Cost |
| --- | --- | --- | --- |
| Secret (development signer) | the cluster | steal the key | none |
| **OpenBao transit** | OpenBao, non-exportable | request signatures while acting as notaio, audited outside the cluster; never steal the key | one outbound path and an auth role |
| Sign in CI, cluster only verifies | the CI system | nothing | compilation moves out of the controller; Sigstore verification is heavy for mediatore-guest |
| Cloud KMS or HSM | the vendor or device | as OpenBao | vendor or device lock-in, at odds with the foundation-friendly rule |

## Decision

1. **The production signer is an OpenBao transit key** of type `ed25519`, created non-exportable.
   A new `Signer` implementation calls `transit/sign/<key>` with the DSSE pre-authentication
   encoding and returns the raw signature. The private key never exists in notaio's memory, on its
   disk, or anywhere in the cluster.
2. **notaio authenticates with OpenBao's Kubernetes auth method,** bound to the
   `notaio-system/notaio` ServiceAccount, with short-lived tokens. Its OpenBao policy allows
   `update` on `transit/sign/<key>` and nothing else: no read of the key's private part, no
   export, no rotation, no configuration.
3. **The key id names the key version,** `<key name>:v<N>`, so a rotation is visible in every
   envelope. Consumers are configured with a set of trusted public keys, taken from OpenBao by an
   operator and distributed out of band, which lets the old and new keys overlap during a rotation.
4. **Rotation and audit belong to OpenBao.** Rotation is an operator action outside the cluster.
   OpenBao's audit device records every signature request.

   *Amended 2026-10-30:* notaio signed with the key's latest version, so a rotation started using
   a version before any consumer could trust it. `NOTAIO_OPENBAO_KEY_VERSION` now pins the version
   notaio signs with: it is sent with every sign request, and a signature from any other version
   is refused, so nothing is published under a key consumers were not given. A rotation is then
   staged: rotate, give consumers the new public key, move the pin. Unset, notaio signs with the
   latest, as before.
5. **The development signer stays for development and the e2e only.** A production deployment does
   not set `NOTAIO_SIGNING_KEY_FILE`. Removing it from the default manifests is ROADMAP M2.
6. *Added 2026-11-01 (roadmap 02):* **OpenBao unseals itself; the Shamir key becomes a recovery
   key.** A Shamir-sealed OpenBao stays sealed after every restart until a person unseals it, and
   once consumers enforce bundles an expired one refuses their claims, so a restart at night is an
   outage by morning. Which seal unwraps OpenBao's root key is a custody decision of its own: it
   decides who, besides OpenBao, can make it sign. Recommended, and what `dev/openbao` does: the
   `static` seal, its key in a Secret in OpenBao's namespace, mounted read-only into the OpenBao pod
   alone. The production shape is the `transit` seal against an OpenBao outside the cluster, or a
   KMS; the [signer guide](../guides/openbao-signer.md#unsealing-after-a-restart) compares them.

## Consequences

- notaio gains one credential (a short-lived OpenBao token, obtained from its ServiceAccount) and
  one outbound path, to OpenBao over TLS. `deploy/controller/30-networkpolicy.yaml` gains one egress
  rule. CLAUDE.md's "no hypervisor credentials" still holds.
- **Accepted risk:** a cluster administrator who can act as notaio's ServiceAccount (mint its
  token, or run a pod as it) can request signatures for arbitrary payloads while that access lasts.
  They cannot take the key away with them, every request is in OpenBao's audit log outside the
  cluster, and the auth role binds the ServiceAccount, namespace and token audience. *Revisit
  when* the platform needs protection from a malicious cluster administrator rather than a careless
  or compromised one: that means signing outside the cluster (the CI option above).
- **Accepted risk (decision 6, the `static` seal):** the seal key sits in the cluster's datastore
  beside the storage it unwraps. Whoever can read Secrets in OpenBao's namespace, or holds a backup
  of both the cluster's datastore and OpenBao's storage, can bring OpenBao up and sign as notaio,
  outside its audit device's reach if they run their own copy. On a single-maintainer test cluster
  that person is already cluster-admin. *Revisit when* the cluster has other administrators or
  other users: then the `transit` seal, its key outside the cluster.
- OpenBao becomes a dependency of publishing. If it is unreachable, notaio cannot sign, so no new
  bundle is published and consumers keep the last good one until it expires. That is fail closed,
  and it makes OpenBao availability part of the bundle lifetime budget.
- The HTTP client for OpenBao is a new dependency of the `notaio` crate only. It must pass
  `deny.toml`, and `sandboxpolicy-bundle` stays free of it.
- OpenBao is a Linux Foundation project, so this satisfies "no HashiCorp tooling for new
  decisions". Its MPL-2.0 licence does not reach notaio: notaio calls its API and links none of it.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
