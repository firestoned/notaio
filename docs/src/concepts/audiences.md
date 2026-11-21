---
status: implemented
---

# SandboxAudience

A **SandboxAudience** is the definition of one audience: how the consumer obtains its credential,
where it places it, which endpoints it is bound to and the widest any grant of it may be. The other
two halves of an audience already live in notaio: the [ExposureCeiling](ceilings.md) says what
Security allows, and the [SandboxPolicy](policies.md) grants it to sandboxes. The definition sits
beside them, compiled and signed the same way ([ADR-0020](../adr/0020-sandbox-audience.md)).

Audience definitions are **off** until the controller's `NOTAIO_AUDIENCE_PLATFORM_NAMESPACE` is
set. While it is unset, notaio ignores `SandboxAudience` objects and checks policies exactly as
before.

## Platform and team audiences

The namespace decides, never a field:

| | Platform audience | Team audience |
| --- | --- | --- |
| Lives in | the platform audience namespace | any other namespace |
| Usable from | any namespace a signed grant reaches | its own namespace only |
| Granted by | any policy | a policy in its own namespace only (L027) |
| Backend | any: `sts`, `secret`, `delegated`, `entra-obo`, `none` | `secret` or `none` (L022) |
| Credential source | the platform's OpenBao, or a Secret in its namespace | a Secret in its own namespace, never OpenBao (L023) |
| Placement `proxy` | allowed | allowed, with a warning (L029) |

`secretRef` and `caRef` have no namespace field: a reference always resolves in the audience's own
namespace. notaio never reads the Secret; it signs the reference, and the consumer resolves it.

## What notaio checks

| Code | Rule |
| --- | --- |
| L017 | Each endpoint is an exact `host:port` name (no wildcard, no IP literal), each rule a method and a path |
| L022 | A team audience's backend is `secret` or `none` |
| L023 | A team audience never reads OpenBao |
| L024 | A team audience and a platform audience never share a name |
| L025 | Within what one namespace can see, an endpoint belongs to one audience |
| L026 | `header` and `extraHeaders` names are lowercase tokens and none of the reserved framing headers; `extraHeaders` values are printable ASCII with no line break |
| L029 | A team audience in the proxy is a warning: its credential sits in the pool VM's memory |

And on every policy that grants an audience:

| Code | Rule |
| --- | --- |
| L027 | The audience has a definition the policy can see: a platform audience, or a team audience in the policy's own namespace |
| L028 | The grant's `endpoints` are a subset of the definition's (empty reaches nothing), its `rules` sit inside the definition's (empty means the definition's), its token lifetime is under `maxTtlSeconds` |
| L014 | The definition's endpoints and `maxTtlSeconds` are approved by the policy's ceiling, so an audience cannot open what Security did not approve |

**Collisions refuse the newcomer** (L024, L025). notaio reads each audience's standing from its
status: *new* (never published), *stable* (its published bundle is its current content) or
*changed* (published, then edited).

- A new audience always loses, and never refuses another. So a platform audience added later
  never breaks a team's working audience, and a refused audience left in place cannot block the
  next edit of the audience it collides with.
- A changed audience gives way to a stable one: an edit that moves an audience into a collision is
  refused, while its last bundle serves out its lifetime.
- Between two changed audiences, a platform audience is kept and the team audience refused; two of
  the same kind are both refused.

At least one side of every collision is refused, so two colliding definitions are never both
published. `notaio lint` sees no status, so in a pull request it reports every collision on both
sides.

`extraHeaders` values are published in the bundle as written. Never put a credential in one: that
is what `source` is for.

## The signed audience bundle

Each valid `SandboxAudience` is compiled into its own DSSE envelope, payload type
`application/vnd.firestoned.sandboxaudience.bundle.v1+json`, schema
`sandbox.firestoned.io/audience-bundle/v1`, signed with the policy bundles' key. Because DSSE signs
the payload type, a policy bundle never verifies as an audience bundle, nor the reverse. It is
published to the ConfigMap `<audience>-audience` in the audience's namespace, under
`audience.dsse.json`, with the same version and digest annotations as a policy bundle, and owned by
the `SandboxAudience`: deleting the audience deletes the bundle.

A finding publishes nothing new. The last valid bundle stays until its lifetime ends, as for a
policy. Consumers verify it with `verify_audience_bundle` and keep a version floor per audience
namespace and name with `floor::accept_audience` ([ADR-0008](../adr/0008-consumer-version-floor.md)).

## Status

```sh
kubectl -n team-a get sandboxaudience artifactory
```

```text
NAME          BACKEND   PLACEMENT   VERSION   READY
artifactory   secret    gateway     3         True
```

`Valid` carries the findings with their codes, `Ready` says whether the bundle was published, and
`bundleVersion`, `bundleDigest` and `bundleNotAfter` describe what consumers hold.

## Next

- [Bring your own credential](../guides/bring-your-own-credential.md), for a team.
- [Set up platform audiences](../guides/platform-audiences.md), for the platform team.
