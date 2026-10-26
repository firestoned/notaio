# Rotate the signing key

OpenBao transit keeps every version of a key. notaio signs with the latest one, and the key id in
each bundle names the version (`notaio:v<N>`), so consumers can trust old and new versions side by
side during a rotation.

## How long a rotation takes to land

notaio does not re-sign on rotation. Each bundle moves to the new version the next time it is
signed: when its policy changes, or at its half-life refresh. With the default one-hour lifetime,
every bundle is signed by the new version within about 30 minutes, and every bundle signed by the
old version has expired within about 90.

## Steps

1. **Rotate.**

    ```sh
    bao write -f transit/keys/notaio/rotate
    ```

2. **Read the new public key at once, and add it to every consumer's trusted keys**, keeping the
   old one:

    ```sh
    bao read -format=json transit/keys/notaio \
      | jq -r '.data.keys | to_entries | max_by(.key | tonumber) | "notaio:v\(.key) \(.value.public_key)"'
    ```

3. **Watch the bundles move.** A bundle's signature key id is in its envelope:

    ```sh
    kubectl -n team-docs get configmap docs-agent-bundle \
      -o jsonpath='{.data.bundle\.dsse\.json}' | jq -r '.signatures[].keyid'
    ```

4. **Retire the old version** once every bundle has moved and one bundle lifetime has passed since:
   remove it from consumers' trusted keys, and stop OpenBao signing with it:

    ```sh
    bao write transit/keys/notaio/config min_encryption_version=<new version>
    ```

## The gap, and how to keep it short

!!! warning "Known limitation"
    notaio cannot be told to keep signing with an older key version, so the new public key cannot
    be staged at consumers before notaio starts using it. Between step 1 and the moment a consumer
    trusts the new version, that consumer refuses newly signed bundles. It keeps verifying bundles
    signed by the old version until they expire, at most one bundle lifetime.

To keep that window harmless, do step 2 well within one bundle lifetime of step 1, ideally from
the same runbook or automation. Pinning the signing key version from the controller would remove
the gap; it is not built.

## If the key is compromised

Rotation does not revoke: a bundle signed by the compromised version stays valid until its
`notAfter`. Rotate, distribute the new key, and remove the compromised version from consumers'
trusted keys **immediately**, accepting that consumers will refuse bundles until they have the new
key. Then find out how the signature was obtained: OpenBao's audit log records every
`transit/sign/notaio` request and the identity behind it.
