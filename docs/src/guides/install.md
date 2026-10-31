# Install notaio

This installs a release into a cluster: the four CRDs, the controller, its RBAC and
NetworkPolicy, and the built-in profiles. It takes about fifteen minutes once OpenBao is ready.

## Before you start

- **The management cluster.** notaio is part of AgentSandbox's trusted control plane, next to
  banlieue and mediatore. Never install it in a general-purpose cluster that runs other workloads.
- **Kubernetes 1.30 or later.** The GitOps-only admission policy is a ValidatingAdmissionPolicy,
  GA in 1.30. notaio is tested on kind (1.31) and k0s (1.36).
- **An OpenBao for the signer**, reachable from the cluster over TLS, set up as in
  [Set up the OpenBao signer](openbao-signer.md). The controller does not start without its CA.
- **A GitOps reconciler** (Flux, Argo CD) that will apply your policy repository. You need its
  username, for example `system:serviceaccount:flux-system:kustomize-controller`.
- **A network plugin that enforces NetworkPolicy**, for the controller's egress and ingress rules
  to mean anything.

## 1. Download and verify the release

Each release attaches two manifests ([ADR-0009](../adr/0009-release-and-supply-chain.md)):
`notaio-<version>-crds.yaml`, the four CustomResourceDefinitions, and `notaio-<version>.yaml`, the
controller and the built-in profiles with the image pinned by digest. They are separate because
the profiles need the CRDs to exist before `kubectl` can even read them. Verify both before you
apply them:

```sh
V=0.3.1
gh release download "v$V" --repo firestoned/notaio \
  -p "notaio-$V*.yaml" -p "notaio-$V*.yaml.sigstore.json" -p checksums.sha256
sha256sum -c checksums.sha256 --ignore-missing

ID='^https://github.com/firestoned/notaio/.github/workflows/build.yaml@refs/tags/v.+$'
for f in "notaio-$V-crds.yaml" "notaio-$V.yaml"; do
  cosign verify-blob "$f" --bundle "$f.sigstore.json" \
    --certificate-identity-regexp "$ID" \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
done

IMAGE=$(grep -o 'ghcr.io/firestoned/notaio@sha256:[0-9a-f]*' "notaio-$V.yaml")
cosign verify "$IMAGE" --certificate-identity-regexp "$ID" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
gh attestation verify "oci://$IMAGE" --repo firestoned/notaio
```

!!! note "Cosign 3 or later"
    Images are signed by Cosign 3, which stores signatures as OCI referrers in the new bundle
    format. Cosign 2 does not look there and reports `no signatures found`, which reads like an
    unsigned image. Check `cosign version` first.

!!! note "`gh attestation verify` says the token was denied"
    `gh` reads ghcr.io through your Docker credential store. A stale ghcr login there is refused
    even though the image is public. Run it with an empty Docker config, which reads anonymously:
    `DOCKER_CONFIG=$(mktemp -d) gh attestation verify ...`

The manifests hold `deploy/crds/`, then `deploy/controller/` and `deploy/profiles/`, from the
tagged commit. They do not hold two things you adapt yourself, both taken from the same tag in
step 5: the bundle Role for each of your policy namespaces, and the admission policy, which names
your GitOps identity.

## 2. Edit for your cluster

Three things in the manifest are site specific. Change them in your copy, or patch them with
kustomize from your GitOps repository.

| What | Where | Change to |
| --- | --- | --- |
| OpenBao address | Deployment env `NOTAIO_OPENBAO_ADDR` | your OpenBao's `https://` URL |
| API server and OpenBao egress | NetworkPolicy `notaio`, egress rules | your control plane CIDR, and where OpenBao runs |
| Who may scrape metrics | NetworkPolicy `notaio`, the ingress rule's namespace label | the namespace your Prometheus runs in |

The rest of the Deployment environment is described in
[the controller section of the architecture](../architecture.md#the-controller).

## 3. Create the namespace and the OpenBao CA

```sh
kubectl apply -f - <<'YAML'
apiVersion: v1
kind: Namespace
metadata:
  name: notaio-system
  labels:
    app.kubernetes.io/name: notaio
    pod-security.kubernetes.io/enforce: restricted
YAML
kubectl -n notaio-system create configmap openbao-ca --from-file=ca.crt=<OpenBao CA certificate>
```

This is the same Namespace the release manifest holds; it is created first only so the ConfigMap
can go in before the controller starts. The CA in `openbao-ca` is the only root the controller
trusts for OpenBao, never the system store.

## 4. Apply the release

The CRDs first, and wait for them; then the rest:

```sh
kubectl apply -f "notaio-$V-crds.yaml"
kubectl wait --for=condition=Established --timeout=60s -f "notaio-$V-crds.yaml"
kubectl apply -f "notaio-$V.yaml"
kubectl -n notaio-system rollout status deployment/notaio
kubectl get sandboxprofiles
```

With Flux, make the CRDs one Kustomization and the rest another that `dependsOn` it.

Two replicas start. One takes the Lease and leads; the other waits, ready to take over
([ADR-0011](../adr/0011-leader-election.md)):

```sh
kubectl -n notaio-system get lease notaio -o jsonpath='{.spec.holderIdentity}{"\n"}'
```

The four built-in profiles should all show `Valid` `True` within a few seconds.

## 5. Allow bundle writes, and lock down direct edits

**Bundle writes are per namespace.** The controller may write ConfigMaps only where a Role lets it.
`deploy/policy-namespace/notaio-bundles.yaml` is that Role and its RoleBinding, with no namespace
of its own. Apply it in every namespace that will hold SandboxPolicy objects:

```sh
RAW=https://raw.githubusercontent.com/firestoned/notaio/v$V
curl -fsSLO "$RAW/deploy/policy-namespace/notaio-bundles.yaml"
kubectl create namespace sandboxes                        # each policy namespace, if new
kubectl -n sandboxes apply -f notaio-bundles.yaml
```

With GitOps, put it in each policy namespace's kustomization instead.

**Direct edits are refused.** Take `deploy/admission/gitops-only.yaml` from the same tag, replace
the allowed username with your GitOps reconciler's, and apply it:

```sh
curl -fsSLO "$RAW/deploy/admission/gitops-only.yaml"
# edit the allowed username, then:
kubectl apply -f gitops-only.yaml
```

From then on only that identity can create, change or delete the four kinds, cluster-admin
included. notaio's own writes go to the status subresource, which the policy does not match.

## 6. Check it

```sh
kubectl -n notaio-system port-forward deploy/notaio 8080:8080 &
curl -s localhost:8080/readyz                                  # ready
curl -s localhost:8080/metrics | grep -E 'fail_open|past_expiry'  # all 0
kill %1
```

Then follow [Write a policy](write-a-policy.md) to publish the first bundle, and
[Observe notaio](observability.md) to wire up the alerts.

## Upgrading

Apply the next release's manifest the same way. The CRDs are alpha; a change to their schema or to
the bundle schema is called out in the release notes and in an ADR.

!!! warning "Upgrading to v1alpha2 (ADR-0013)"
    v1alpha2 replaces v1alpha1, which is retired rather than converted: serving both would need a
    conversion webhook. Before applying the release, rewrite the policy repository's objects as
    v1alpha2 (`apiVersion`, audience `endpoints` and `rules`, direct `egress` as
    `{ endpoint, rules }`, ceiling audience `endpoints`) and check them with `notaio lint`, which
    refuses a v1alpha1 file with `I009`. Then apply the release's CRDs and let GitOps re-apply the
    objects. Bundle versions never go
backwards across an upgrade: the controller takes the higher of the status and the published
ConfigMap's annotation.
