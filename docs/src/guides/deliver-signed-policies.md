---
status: implemented
---

# Deliver policies as signed OCI artifacts

With plain GitOps, whatever the policy repository's default branch holds reaches the cluster. This
guide replaces that with two signed OCI artifacts that Flux verifies before applying anything
([ADR-0015](../adr/0015-signed-policy-delivery.md)):

| Artifact | Holds | Flux requires |
| --- | --- | --- |
| `policies` | the namespaces, notaio's bundle Role, the SandboxPolicies | CI's keyless signature, from the default branch, after `notaio lint` |
| `ceilings` | the ExposureCeilings | Security's signature, with a key only Security holds |

A push credential or a compromised workflow can then ship a policy, but never a ceiling, and notaio
never signs a policy above its ceiling. notaio's own bundle signature is unchanged: this secures what
reaches the cluster, the bundle signature what consumers enforce
([The Bundle](../concepts/bundle.md#what-the-signature-does-not-cover)).

The examples use GitHub Actions, the GitHub container registry and Flux 2.10. Replace
`example-org/policy-repo` with your repository.

## Before you start

- A policy repository linted on every pull request: [Lint a policy repository](lint-in-ci.md).
- Flux on the cluster, applying that repository today, with GitOps-only admission naming its
  `kustomize-controller` ([Install notaio](install.md), step 5).
- Cosign 3 and the Flux CLI where Security signs.

## 1. CI publishes both artifacts

Add a job that runs on the default branch after lint. It renders each namespace as Flux would apply
it, splits the output by kind, pushes both artifacts, and signs them keyless by digest. The
`ceilings` artifact is pushed only when its content changes, tagged by a hash of that content, so
Security signs only when there is something new.

```yaml
  publish:
    needs: lint
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write   # push to ghcr.io
      id-token: write   # keyless signatures
    env:
      REPO: ghcr.io/${{ github.repository }}
    steps:
      - uses: actions/checkout@<pinned sha>
      - uses: fluxcd/flux2/action@<pinned sha of the Flux release>
        with: { version: 2.10.0 }
      - uses: sigstore/cosign-installer@<pinned sha>
      - uses: docker/login-action@<pinned sha>
        with: { registry: ghcr.io, username: "${{ github.actor }}", password: "${{ github.token }}" }
      - name: Render and split
        run: |
          mkdir -p stage/policies stage/ceilings
          for dir in namespaces/*/; do
            ns=$(basename "$dir")
            kubectl kustomize "$dir" > "stage/$ns.all.yaml"
            yq 'select(.kind == "ExposureCeiling")' "stage/$ns.all.yaml" > "stage/ceilings/$ns.yaml"
            yq 'select(.kind != "ExposureCeiling")' "stage/$ns.all.yaml" > "stage/policies/$ns.yaml"
          done
      - name: Push and sign policies
        run: |
          out=$(flux push artifact "oci://$REPO/policies:${GITHUB_SHA::12}" --path=stage/policies \
            --source="$(git config --get remote.origin.url)" \
            --revision="${GITHUB_REF_NAME}@sha1:${GITHUB_SHA}" 2>&1)
          digest=$(grep -o 'sha256:[0-9a-f]\{64\}' <<<"$out" | tail -1)
          flux tag artifact "oci://$REPO/policies:${GITHUB_SHA::12}" --tag latest
          cosign sign --yes "$REPO/policies@$digest"
      # ceilings: the same, tagged content-<hash of stage/ceilings>, skipped when that tag exists,
      # and signed keyless as provenance; Security adds the signature Flux requires.
```

Sign by digest: Cosign 3 deprecates signing a tag. The packages are private when the repository
is; Flux needs a token to read them (step 3).

## 2. Security signs the ceilings

Once, Security makes its key pair and commits only the public half:

```sh
cosign generate-key-pair --output-key-prefix security    # asks for a password
mkdir -p keys && mv security.pub keys/security.pub         # keep security.key out of the repository
```

Then, for each new `ceilings` digest, a Security member on their own machine checks that CI built
it, reads it, and signs it, with no transparency log entry:

```sh
REF=ghcr.io/example-org/policy-repo/ceilings@sha256:<digest>

cosign verify "$REF" \
  --certificate-identity-regexp '^https://github.com/example-org/policy-repo/.github/workflows/<workflow>.yaml@refs/heads/main$' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com > /dev/null
flux pull artifact "oci://$REF" --output ./review && cat ./review/*.yaml

cosign signing-config create --out signing-config.json    # no service: nothing is published
cosign sign --yes --key "$SECURITY_KEY" --signing-config signing-config.json "$REF"
cosign verify "$REF" --key keys/security.pub --insecure-ignore-tlog=true > /dev/null
```

Keep this in a script in the policy repository, so every signature follows the same steps.

!!! warning "Not `COSIGN_KEY`"
    Cosign reads a `COSIGN_KEY` environment variable as `--key` on every command. Set in the same
    shell, it turns the keyless check of CI's signature into a key check, which fails. Name the
    variable something else.

`verify` with `--insecure-ignore-tlog` warns that skipping the transparency log is insecure.
Security's signatures have no log entry by design, so that warning is expected.

## 3. Flux verifies the sources

Two Secrets in `flux-system`, created by hand, never in git:

```sh
kubectl -n flux-system create secret docker-registry policy-artifacts \
  --docker-server=ghcr.io --docker-username=<user> --docker-password=<token with read:packages>
kubectl -n flux-system create secret generic cosign-security --from-file=cosign.pub=keys/security.pub
```

Then the sources, in the repository Flux syncs its own configuration from:

```yaml
apiVersion: source.toolkit.fluxcd.io/v1
kind: OCIRepository
metadata: { name: policies, namespace: flux-system }
spec:
  interval: 5m
  url: oci://ghcr.io/example-org/policy-repo/policies
  ref: { tag: latest }
  secretRef: { name: policy-artifacts }
  verify:
    provider: cosign
    matchOIDCIdentity:
      - issuer: ^https://token\.actions\.githubusercontent\.com$
        subject: ^https://github\.com/example-org/policy-repo/\.github/workflows/<workflow>\.yaml@refs/heads/main$
---
apiVersion: source.toolkit.fluxcd.io/v1
kind: OCIRepository
metadata: { name: ceilings, namespace: flux-system }
spec:
  interval: 5m
  url: oci://ghcr.io/example-org/policy-repo/ceilings
  ref: { tag: latest }
  secretRef: { name: policy-artifacts }
  verify:
    provider: cosign
    secretRef: { name: cosign-security }
```

Merge these alone first and check both report `Ready=True` and `SourceVerified=True`. Keyless
verification needs source-controller to reach Sigstore's public trust root, which Flux's default
egress policy allows.

## 4. Move the namespaces over, without a gap

Done in this order, nothing is deleted or recreated and no policy changes version:

1. Add a Kustomization per source (`path: ./`, `prune: true`; the ceilings one `dependsOn` the
   policies one, whose artifact holds the namespace), and set `prune: false` on the Kustomization
   that applies the namespaces from git. Both now apply the same objects; the new ones take them
   over under the same `kustomize-controller` identity.
2. Check the objects carry the new Kustomizations' `kustomize.toolkit.fluxcd.io/name` labels, and
   every policy is still `Ready=True, Published` at the same version.
3. Remove the git-based Kustomization. Its prune is off, so deleting it deletes nothing.

The git source stays if Flux's own configuration and the admission policy still come from git.

## Check it

| Try | Expect |
| --- | --- |
| A policy change merged | CI signs `policies`; Flux applies it; notaio publishes the next version |
| A ceiling change merged, not yet signed by Security | the `ceilings` source `SourceVerified=False`, `no matching signatures were found`; the previous ceiling stays applied |
| Security signs it | applied at the next interval |
| An artifact pushed to `policies:latest` without CI | `SourceVerified=False`, `no signatures found`; the last signed one stays applied |

A refused source leaves the cluster as it was: Flux keeps the last verified artifact and applies
nothing new. Restore a tag that was moved to an unsigned digest with `flux tag artifact
oci://<artifact>:<signed tag> --tag latest`, or let CI's next run move it.
