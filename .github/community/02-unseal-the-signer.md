# Roadmap 02: OpenBao unseals itself

- **Status:** Not started. The decision below is recommended, not yet taken.
- **Milestone:** M3 in [`ROADMAPS.md`](../../ROADMAPS.md): before a consumer enforces bundles signed
  on the test cluster (roadmap 00, phase 2)
- **Related:** roadmap 01 (section 3, "OpenBao stays the test instance", and its risk "the test
  OpenBao seals on every restart"; test D3); ADR-0004 (the signer); the
  [OpenBao signer guide](../../docs/src/guides/openbao-signer.md)

## 1. Goal

A restarted OpenBao is signing again within a minute, with nobody at a keyboard. Today it comes back
sealed, and stays sealed until a person unseals it: roadmap 01's test D3 showed notaio handles that
correctly (`SigningFailed`, the published bundle valid until its `notAfter`, recovery on unseal),
but a bundle lifetime is an hour, so a restart at night is an outage by morning. That was fine while
nothing depended on the bundles. Once mediatore enforces them (roadmap 00, phase 2), an expired
bundle refuses claims and mints.

## 2. Where things stand

| Piece | State |
| --- | --- |
| OpenBao | 2.7.1, one replica, PostgreSQL storage in the same cluster, TLS 1.3, file audit to stdout |
| Seal | Shamir, one key share, threshold one; the unseal key and root token kept off the cluster, readable only by the maintainer |
| What it signs | notaio's bundles (transit, Kubernetes auth bound to notaio's ServiceAccount, ADR-0004) |
| Seals the 2.7.1 binary carries | `static` (a key from a file or the environment), `transit` (another OpenBao or Vault), `awskms`, `gcpckms`, `azurekeyvault`; no `ocikms`; PKCS#11 not confirmed |

## 3. Decision

What unwraps OpenBao's root key when it starts. In every option the Shamir key becomes a recovery
key, kept where the unseal key is kept today: it is still needed for a few operator actions, never
for a restart.

- **Recommended: the `static` seal, its key in a Kubernetes Secret in OpenBao's namespace,**
  mounted read-only into the pod and readable by nothing else (no RoleBinding grants Secrets
  there). No new service, no cloud account, no network dependency at start. What it costs: the key
  sits in the cluster's datastore, beside the storage it unwraps, so whoever can read Secrets in
  that namespace, or holds a backup of both the datastore and OpenBao's PostgreSQL, holds the
  signing key. On a single-maintainer test cluster that person is already cluster-admin; the
  datastore backup is encrypted. The key rotates through `previous_key_id`.
- *Alternative: the `transit` seal against a second OpenBao outside the cluster* (on the host
  that runs it), itself sealed by Shamir or `static` from a root-only file. The key leaves the
  cluster, at the price of a second OpenBao to run, its TLS, and a renewable token in the cluster.
  The manual unseal moves to that instance, which restarts only with its host. The production
  shape, and the next step if the test cluster grows other users.
- *Alternative: a cloud KMS* (`awskms`, `gcpckms`, `azurekeyvault`). Real separation, but a cloud
  account, its credentials in the cluster, and a network dependency for every start, for a test
  cluster that has none of these today.
- *Rejected: a sidecar that types the Shamir key in* from a Secret. The same exposure as the
  `static` seal, with more moving parts and the unseal key itself in the cluster.

The decision is recorded as an amendment to ADR-0004 (who holds what, decision on the signer's
custody), with the residual risk in the threat model.

## 4. Work, in order

### Phase A: prove it on kind

- [ ] The kind OpenBao of `e2e_openbao` on the `static` seal: initialise, write a transit key,
      restart the pod, and it is unsealed and signs, with no unseal step.
- [ ] A migration from Shamir, as the test cluster will do it: a Shamir-sealed instance with a
      transit key, the seal stanza added, a restart, `bao operator unseal -migrate` with the
      Shamir key; the transit key and its versions survive, and notaio's pinned key version
      still signs.
- [ ] A missing or wrong key: OpenBao refuses to start unsealed, and says why. A pod that cannot
      unseal is not a pod that signs with something else.

### Phase B: the test cluster

- [ ] Before: a dump of OpenBao's PostgreSQL and the current unseal material, kept off the
      cluster.
- [ ] A 32-byte random key in a Secret, the seal stanza in the config, the Deployment mounting
      it; the rollout, then the migration with the Shamir key. `bao status`: `Seal Type static`,
      `Recovery Seal`.
- [ ] Delete the pod: unsealed again without anyone, notaio signs, `make cluster-smoke` passes.
      The same test as roadmap 01's D3, with the unseal step gone.
- [ ] The commands and results in the policy repository's `TESTING.md`; here, the results without
      names.

### Phase C: documentation

- [ ] The OpenBao signer guide: an unsealing section (the seal options, the recommended one, the
      migration, rotating the seal key), replacing "a restarted OpenBao is sealed until someone
      unseals it".
- [ ] Troubleshooting: `OpenBao answered 503` gains the auto-unseal case (the seal key missing or
      wrong).
- [ ] ADR-0004 amended and the threat model's residual risk.

## 5. Done when

The test cluster's OpenBao pod is deleted and, with nobody acting, notaio publishes the next bundle
version within a minute, and the smoke test passes.

## 6. Risks

- **The seal key is the signing key's last line.** Anyone who reads it and holds the storage can
  sign as notaio. The Secret is readable by nothing but OpenBao's pod and cluster-admin, and the
  storage dump is kept off the cluster; the transit alternative removes the first half.
- **A migration gone wrong locks the data.** Phase A rehearses it, and phase B starts from a dump
  that a Shamir instance can be restored from.
- **One replica.** Auto-unseal makes a restart short, not absent: signing pauses while the pod
  starts. Bundles already published stay valid until their `notAfter`.
