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

A restarted OpenBao is sealed until someone unseals it; notaio retries and recovers on its own.
