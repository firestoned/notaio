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

Each release attaches `notaio-<version>.yaml`, with the image pinned by digest
([ADR-0009](../adr/0009-release-and-supply-chain.md)). Verify it before you apply it:

```sh
V=0.1.0
gh release download "v$V" --repo firestoned/notaio \
  -p "notaio-$V.yaml" -p "notaio-$V.yaml.sigstore.json" -p checksums.sha256
sha256sum -c checksums.sha256 --ignore-missing

ID='^https://github.com/firestoned/notaio/.github/workflows/build.yaml@refs/tags/v.+$'
cosign verify-blob "notaio-$V.yaml" --bundle "notaio-$V.yaml.sigstore.json" \
  --certificate-identity-regexp "$ID" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com

IMAGE=$(grep -o 'ghcr.io/firestoned/notaio@sha256:[0-9a-f]*' "notaio-$V.yaml")
cosign verify "$IMAGE" --certificate-identity-regexp "$ID" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
gh attestation verify "oci://$IMAGE" --repo firestoned/notaio
```

The manifest holds `deploy/crds/`, `deploy/controller/` and `deploy/profiles/` from the tagged
commit. It does not hold the admission policy, which names your GitOps identity (step 5).

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

```sh
kubectl apply -f "notaio-$V.yaml"
kubectl -n notaio-system rollout status deployment/notaio
kubectl get sandboxprofiles
```

Two replicas start. One takes the Lease and leads; the other waits, ready to take over
([ADR-0011](../adr/0011-leader-election.md)):

```sh
kubectl -n notaio-system get lease notaio -o jsonpath='{.spec.holderIdentity}{"\n"}'
```

The four built-in profiles should all show `Valid` `True` within a few seconds.

## 5. Allow bundle writes, and lock down direct edits

**Bundle writes are per namespace.** The controller may write ConfigMaps only where a Role lets it.
For every namespace that will hold SandboxPolicy objects, copy the `notaio-bundles` Role and
RoleBinding from `deploy/controller/10-rbac.yaml` and change its namespace from the example,
`sandboxes`.

**Direct edits are refused.** Take `deploy/admission/gitops-only.yaml` from the same release,
replace the allowed username with your GitOps reconciler's, and apply it:

```sh
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

Apply the next release's manifest the same way. The CRDs are `v1alpha1`; a change to their schema
or to the bundle schema is called out in the release notes and in an ADR. Bundle versions never go
backwards across an upgrade: the controller takes the higher of the status and the published
ConfigMap's annotation.
