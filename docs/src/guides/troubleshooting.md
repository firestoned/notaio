# Troubleshooting

Start from the policy's `Ready` condition: its reason says which gate stopped it.

```sh
kubectl -n <ns> get sandboxpolicy <name> \
  -o jsonpath='{range .status.conditions[*]}{.type}={.status} {.reason}: {.message}{"\n"}{end}'
kubectl -n <ns> describe sandboxpolicy <name>     # the same, plus recent Events
```

## By `Ready` reason

| Reason | What it means | What to do |
| --- | --- | --- |
| `Published` | Signed and published. | Nothing. If a consumer still has an older version, look at the consumer. |
| `ProfileNotFound` | `spec.profileRef` names no SandboxProfile. | Fix the name; `kubectl get sandboxprofiles` lists them. |
| `InvalidProfile` | The profile fails lint. | `kubectl describe sandboxprofile <name>`: its `Valid` condition has the findings. Fix the profile; every policy using it recovers. |
| `InvalidPolicy` | The policy breaks a rule against its effective knobs. | The message lists each finding with its code and field. Run `notaio lint` on the file. |
| `NoApprovedCeiling` | No ExposureCeiling with the policy's name and namespace. | Add one, approved by Security: [Approve an exposure ceiling](approve-a-ceiling.md). |
| `CeilingNotApproved` | The ceiling has no `Security` approval. | Add the approval, with its reference. |
| `ExposureAboveCeiling` | The policy, grants included, reaches more than its ceiling. | Compare `status.maxExposure` with the ceiling; narrow the policy, or widen the ceiling with Security. A new grant is a common cause. |
| `NotCanonical` | The content has a number RFC 8785 cannot represent exactly. | Look for a value above 2^53 or a fraction; budgets are the usual place. |
| `NoSigner` | No signer is configured. | The Deployment has neither `NOTAIO_OPENBAO_ADDR` nor a development key. See [Install](install.md). |
| `SigningFailed` | OpenBao refused or could not be reached. The previous bundle stays. | See below. |
| `FailOpenRefused` | The fail-open guard caught a bundle the evaluator should never have produced. | A bug in notaio: [report it](../security/index.md) with the policy, its grants and ceiling. |

## A grant is not applied

```sh
kubectl -n <ns> get knobgrants
kubectl -n <ns> describe knobgrant <name>     # the Applied condition has the findings
```

`Rejected` lists the codes: `L011` (not exactly one step, or the knob is already relaxed), `L012`
(expires too far out), `L013` (an approval missing), or `L003` (it would complete a toxic
combination). `Pending` usually means its `policyRef` names no policy in the namespace.

## Signing fails

The policy's `SigningFailed` message is the signer's error, also in the controller's log:

```sh
kubectl -n notaio-system logs deploy/notaio | grep -i openbao
```

| The message starts with | Likely cause |
| --- | --- |
| `cannot reach OpenBao` | The address, the NetworkPolicy egress rule to OpenBao, or TLS: ConfigMap `openbao-ca` must hold the CA that signed OpenBao's certificate. |
| `OpenBao answered 503` | OpenBao is sealed, usually after a restart. Unseal it; notaio recovers on its own. |
| `OpenBao answered 403`, or `OpenBao refused the token` | The Kubernetes auth role does not match (ServiceAccount `notaio`, namespace `notaio-system`, audience `openbao`), or its policy does not allow `update` on `transit/sign/<key>`. The body says which. |
| `signer configuration` | Usually the projected token file is missing: check the Deployment's `openbao-token` volume. |
| `unexpected response from OpenBao` | The key is not ed25519, or a proxy rewrote the answer. |

## The pod does not start or is not ready

| Symptom | Cause |
| --- | --- |
| Stuck in `ContainerCreating`, `FailedMount` Events | ConfigMap `openbao-ca` is missing in `notaio-system`. |
| Exits at start: both signers configured | The Deployment has both `NOTAIO_OPENBAO_ADDR` and `NOTAIO_SIGNING_KEY_FILE`; notaio refuses to guess which key signs. |
| Exits at start: binding the health and metrics listener | Port 8080 is taken, or `NOTAIO_HTTP_ADDR` is invalid. |
| Exits at start: `NOTAIO_POD_NAME is required` | The Deployment lost its downward API env; it is the replica's identity in leader election. |
| Restarts with `stopped leading` in the last log | The leader could not renew its Lease for 10 seconds (the API server was unreachable or slow), or found another holder. It restarts as a follower; one restart is normal after an API server outage. |
| Running, `/readyz` 503 | The controller cannot list a kind: check its RBAC and the NetworkPolicy egress to the API server. |
| `ImagePullBackOff` | The image reference; install from a release manifest, which pins it by digest. |

## Nothing reconciles, though both pods are ready

Only the replica holding the Lease reconciles. See who holds it:

```sh
kubectl -n notaio-system get lease notaio -o jsonpath='{.spec.holderIdentity}{"\n"}'
kubectl -n notaio-system logs <pod> | grep -i lease
```

No holder, and `Lease check failed` in the logs, means neither replica can create or update it:
check the `notaio-leader` Role and RoleBinding in the controller's namespace.

## A policy never gets a status, or keeps an old one

The controller can only write ConfigMaps in namespaces that have the `notaio-bundles` Role and
RoleBinding. Without them the publish fails before the status is written, so a new policy shows no
`Ready` condition at all, `notaio_reconcile_errors_total` rises, and the logs show a 403 on
`configmaps`. See [Install](install.md#5-allow-bundle-writes-and-lock-down-direct-edits).
