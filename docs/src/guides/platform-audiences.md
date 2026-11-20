---
status: implemented
---

# Set up platform audiences

For the platform team. Turning on audience definitions moves each audience's definition out of the
consumer's configuration file and under notaio's signature, and lets teams bring their own
credentials ([ADR-0020](../adr/0020-sandbox-audience.md)).

## 1. Choose the platform audience namespace

One namespace holds the platform's audiences; every other namespace holds team audiences. There is
no default: notaio and every consumer must name the same one, and a mismatch is a refusal rather
than two components guessing. This guide uses `sandbox-platform`.

```sh
kubectl create namespace sandbox-platform
# notaio publishes audience bundles here, as in any policy namespace.
kubectl -n sandbox-platform apply -f deploy/policy-namespace/
```

## 2. Define the platform audiences

```yaml
apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxAudience
metadata:
  name: model-api
  namespace: sandbox-platform
spec:
  credential:
    backend: secret
    placement: gateway
    source:
      openbao: { mount: kv-platform, path: mediatore/model-api, key: api-key }
    header: x-api-key
  endpoints: ["models.bar.foo.io:443"]
  rules:
    - { method: POST, path: "/v1/messages" }
  maxTtlSeconds: 900
```

A platform audience may use any backend, and read OpenBao with the consumer's own identity. Deliver
these from the platform's policy repository, like the ceilings
([Deliver signed policies](deliver-signed-policies.md)).

## 3. Lint the whole set before turning it on

Every policy that grants an audience needs a definition it can see the moment you turn audiences on
(L027), and every definition's endpoints must be in the granting policies' ceilings (L014):

```sh
notaio lint --platform-audience-namespace sandbox-platform policies/ audiences/
```

Fix what it reports first. Turning audiences on with a finding stops that policy publishing; its
last bundle serves until it expires (15 minutes by default).

## 4. Turn it on

Set the variable on the controller (`deploy/controller/20-deployment.yaml` has it commented out):

```yaml
- name: NOTAIO_AUDIENCE_PLATFORM_NAMESPACE
  value: sandbox-platform
```

notaio starts a third reconciler, waits for its cache of every audience to fill, then reconciles.
Each valid audience is published to `<audience>-audience` beside it:

```sh
kubectl get sandboxaudiences -A
```

Then configure each consumer with the same namespace and the same trusted signing key (mediatore's
ADR-0055 has its side, including a shadow period that compares the bundles with its file).

## Onboarding a team

- Give the team's namespace the `notaio-bundles` Role (`deploy/policy-namespace/`).
- Admit only the team's GitOps identity as a writer of `SandboxAudience`, `SandboxPolicy` and the
  bundle ConfigMaps there (`deploy/admission/gitops-only.yaml` shows the pattern with one identity;
  bind one per namespace).
- Give the consumer read on the Secrets the team's audiences name, by `resourceNames`.
- Point the team at [Bring your own credential](bring-your-own-credential.md).

## Adding a platform audience later

A new platform audience that takes a working team audience's name or endpoint is refused (L024,
L025), not the team's: the newcomer always loses. Check `notaio lint` across the teams' audiences
before you add one, and agree a rename where they collide.

## Turning it off

Unset the variable. The audience reconciler stops; existing audience bundles expire after one
lifetime and are not refreshed, and policies are checked as before.
