---
status: implemented
---

# Bring your own credential

For a team that wants its sandboxes to use a credential the team already owns: a service account
for its artifact registry, a token for its own API. You define a **team audience** in your own
namespace, and the credential never passes through the platform team
([ADR-0020](../adr/0020-sandbox-audience.md)).

Your platform must have audience definitions turned on (`NOTAIO_AUDIENCE_PLATFORM_NAMESPACE` set
on notaio, and the same namespace configured on the consumer). Ask your platform team which
namespace that is; yours is any other.

## 1. Put the credential in a Secret in your namespace

Sync it from your own OpenBao with the External Secrets Operator, or create it by any means your
team uses. notaio never reads it; the consumer does, by name, in your namespace only.

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: af-team-a-svc
  namespace: team-a
stringData:
  username: svc-team-a
  token: "<from your secret store>"
```

## 2. Define the audience

```yaml
apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxAudience
metadata:
  name: artifactory
  namespace: team-a
spec:
  credential:
    backend: secret          # a team audience: secret or none (L022)
    placement: gateway       # proxy is allowed, with a warning (L029)
    format: basic            # bearer (default) or basic
    source:
      secretRef: { name: af-team-a-svc, usernameKey: username, tokenKey: token }
  endpoints: ["artifactory.example.com:443"]
  rules:
    - { method: GET,  path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
    - { method: HEAD, path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
    - { method: PUT,  path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
  maxTtlSeconds: 3600
```

The name may not be a platform audience's (L024), and the endpoint may not belong to another
audience your namespace can see (L025). If the platform already defines an audience for a system,
use it rather than defining your own.

## 3. Grant it from a policy in the same namespace

```yaml
apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxPolicy
metadata:
  name: build-agent
  namespace: team-a
spec:
  profileRef: standard
  audiences:
    - name: artifactory
      access: Write
      endpoints: ["artifactory.example.com:443"]   # empty would reach nothing
      rules:
        - { method: GET, path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
      ttlSeconds: 900
```

A grant may narrow the definition, never widen it (L028). Only a policy in `team-a` can grant a
`team-a` audience (L027).

## 4. Ask Security to approve the endpoint

Your policy's [ExposureCeiling](approve-a-ceiling.md) must approve the audience, its endpoints and
its token lifetime, the definition's included (L014). Writing an endpoint into your own audience
opens nothing until Security approves it.

## 5. Check before you merge

```sh
notaio lint --platform-audience-namespace sandbox-platform team-a/
```

Use your platform's namespace name. Without the flag, lint reports `I011` and checks no audience.

## 6. Watch it publish

```sh
kubectl -n team-a get sandboxaudience artifactory
kubectl -n team-a get configmap artifactory-audience -o jsonpath='{.metadata.annotations}'
```

A finding sets `Valid=False` with its code and publishes nothing new; the last valid bundle keeps
serving until it expires.

## Who may write it

Your namespace should admit only your GitOps identity as a writer of `SandboxAudience`,
`SandboxPolicy` and the bundle ConfigMaps (decision 12). The `Secret` is yours: whoever can write it
changes the credential, which notaio does not sign.
