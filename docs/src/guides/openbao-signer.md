# Set up the OpenBao signer

notaio signs bundles with an ed25519 key in OpenBao's transit engine, which never leaves OpenBao
([ADR-0004](../adr/0004-signing-key-custody.md)). notaio logs in with its Kubernetes
ServiceAccount and may ask for signatures from that one key, and nothing else. This guide sets
that up in an OpenBao you run.

!!! tip "Just testing?"
    `make openbao-dev` stands up a complete test OpenBao (PostgreSQL storage, TLS, audit, the key,
    the policy and the role) on a test cluster. See
    [Local Development](../developer/index.md#a-test-openbao-for-the-signer). Never use it for
    production: it has one key share and one replica.

## What notaio needs

| Item | Value |
| --- | --- |
| Transit key | `notaio`, type `ed25519`, `exportable=false` |
| Policy | `update` on `transit/sign/notaio`, nothing else |
| Kubernetes auth role | `notaio`, bound to ServiceAccount `notaio` in `notaio-system`, audience `openbao` |
| Token lifetime | short; the test setup uses 15 minutes |
| Audit | a device on, so every signature request is recorded outside notaio |
| TLS | the CA that signed OpenBao's certificate, for ConfigMap `openbao-ca` |

## 1. The key

```sh
bao secrets enable transit          # if it is not already
bao write -f transit/keys/notaio type=ed25519 exportable=false
```

## 2. The policy

```sh
bao policy write notaio-sign - <<'HCL'
# notaio may request signatures from its key and do nothing else (ADR-0004, decision 2).
path "transit/sign/notaio" { capabilities = ["update"] }
HCL
```

## 3. Kubernetes auth

OpenBao validates notaio's projected token with the cluster's TokenReview API. Configure the
`kubernetes` auth method for your cluster (its API address and CA, and a reviewer identity with
`system:auth-delegator`), then bind the role:

```sh
bao write auth/kubernetes/role/notaio \
  bound_service_account_names=notaio \
  bound_service_account_namespaces=notaio-system \
  audience=openbao \
  token_policies=notaio-sign \
  token_ttl=15m token_max_ttl=15m
```

The Deployment mounts a projected token with audience `openbao` at
`/var/run/secrets/openbao/token`, rotated by the kubelet. If your auth mount, role or key are named
differently, set `NOTAIO_OPENBAO_AUTH_PATH`, `NOTAIO_OPENBAO_ROLE` or `NOTAIO_OPENBAO_KEY` on the
Deployment.

## 4. Give consumers the public key

Consumers verify bundles against the public key, by key id `notaio:v<N>`, where `N` is the key
version:

```sh
bao read -format=json transit/keys/notaio \
  | jq -r '.data.keys | to_entries | max_by(.key | tonumber) | "notaio:v\(.key) \(.value.public_key)"'
```

Distribute that line through whatever configures mediatore, mediatore-guest and the gateway. It is
public; it does not need to be secret, only authentic.

## 5. Check it

Once notaio is installed:

```sh
kubectl -n notaio-system logs deploy/notaio | grep 'signing bundles'
# {"message":"signing bundles","signer":"OpenBao transit key \"notaio\""}
```

After the first policy publishes, OpenBao's audit log shows a `transit/sign/notaio` request from
the `notaio` role, and the bundle's signature key id is `notaio:v1`.

## What happens when OpenBao is down

Nothing new is published. Each affected policy reports `Ready=False, SigningFailed` with the
reason, a `SigningFailed` warning Event is recorded once, and `notaio_signing_failures_total`
rises. Bundles already published stay valid until their `notAfter`, so a short outage is
invisible to consumers. Alert on it (see [Observe notaio](observability.md)).

## Unsealing after a restart

An OpenBao sealed with Shamir keys comes back sealed after every restart, and stays sealed until
someone unseals it: notaio retries and recovers on its own, but a restart at night is an outage
by the time bundles expire. Give OpenBao a seal that unseals it at start. Which one is a custody
decision; the options OpenBao 2.7 carries:

| Seal | The key that unwraps OpenBao lives | Costs |
| --- | --- | --- |
| `static` | in a file or environment variable OpenBao reads at start, typically a Secret beside it | Whoever reads that Secret and holds OpenBao's storage holds the signing key |
| `transit` | in another OpenBao or Vault, outside the cluster | A second instance to run and unseal, and a renewable token |
| `awskms`, `gcpckms`, `azurekeyvault` | in a cloud KMS | A cloud account, its credentials, and a network dependency at every start |

notaio's test instance (`dev/openbao`) uses `static`, its key in Secret `openbao-seal`
(roadmap 02):

```hcl
seal "static" {
  current_key_id = "seal-1"
  current_key    = "file:///seal/seal-1"   # 32 bytes, raw or as 64 hex characters
}
```

- **Moving an instance from Shamir:** add the stanza, restart, then
  `bao operator unseal -migrate <unseal key>`. The transit key and every version of it stay as
  they were; the Shamir key becomes the recovery key, needed for a few operator actions and
  never for a restart. Keep it where the unseal key was kept.
- **A missing key file** stops OpenBao at start (`Error configuring seal "static"`); **a wrong
  key** leaves it sealed, logging `message authentication failed`. Neither signs with anything
  else.
- **Rotating the seal key:** add the new key beside the old, name it in `current_key_id` and
  `current_key`, move the old one to `previous_key_id` and `previous_key`, restart; remove the old
  key once OpenBao has re-wrapped with the new one.
