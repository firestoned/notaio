# Rotate the signing key

OpenBao transit keeps every version of a key. notaio signs with the version you pin, or the latest
when none is pinned, and the key id in each bundle names the version (`notaio:v<N>`), so consumers
can trust old and new versions side by side during a rotation.

## Pin the version first

Set `NOTAIO_OPENBAO_KEY_VERSION` on the controller to the version it signs with now. Unpinned,
notaio signs with the latest version, so a rotation would start signing with a key no consumer
trusts yet. Pinned, it keeps signing with the pinned version, and refuses a signature from any
other ([ADR-0004](../adr/0004-signing-key-custody.md), amended).

```sh
kubectl -n notaio-system set env deployment/notaio NOTAIO_OPENBAO_KEY_VERSION=1
```

Through GitOps, set it in your patch of `deploy/controller/20-deployment.yaml` instead.

## Steps

1. **Rotate.** OpenBao creates the next version; notaio keeps signing with the pinned one.

    ```sh
    bao write -f transit/keys/notaio/rotate
    ```

2. **Add the new public key to every consumer's trusted keys**, keeping the old one. There is no
   hurry: nothing is signed with the new version yet.

    ```sh
    bao read -format=json transit/keys/notaio \
      | jq -r '.data.keys | to_entries | max_by(.key | tonumber) | "notaio:v\(.key) \(.value.public_key)"'
    ```

3. **Move the pin** once every consumer trusts the new version:

    ```sh
    kubectl -n notaio-system set env deployment/notaio NOTAIO_OPENBAO_KEY_VERSION=2
    ```

4. **Watch the bundles move.** notaio does not re-sign on a pin change; each bundle moves the next
   time it is signed, when its policy changes or at its half-life refresh. With the default
   one-hour lifetime, every bundle is signed by the new version within about 30 minutes. A
   bundle's key id is in its envelope:

    ```sh
    kubectl -n team-docs get configmap docs-agent-bundle \
      -o jsonpath='{.data.bundle\.dsse\.json}' | jq -r '.signatures[].keyid'
    ```

5. **Retire the old version** once one bundle lifetime has passed since the last bundle moved:
   remove it from consumers' trusted keys, and stop OpenBao signing with it.

    ```sh
    bao write transit/keys/notaio/config min_encryption_version=2
    ```

At no point does a consumer see a bundle signed by a key it does not trust.

!!! note "Without a pin"
    An unpinned controller starts signing with the new version as soon as you rotate. Consumers
    that do not trust it yet refuse the new bundles, and keep the old ones only until they expire.
    Pin first.

## If the key is compromised

Rotation does not revoke: a bundle signed by the compromised version stays valid until its
`notAfter`. Rotate, move the pin to the new version, distribute the new key, and remove the
compromised version from consumers' trusted keys **immediately**, accepting that consumers will refuse bundles until they have the new
key. Then find out how the signature was obtained: OpenBao's audit log records every
`transit/sign/notaio` request and the identity behind it.
