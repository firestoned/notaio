# SandboxPolicy

A **SandboxPolicy** says who may claim a sandbox, under which [profile](profiles.md), and what the
sandbox may reach: the downstream audiences it can get tokens for, the hosts it can call, the tools
it can use, and how long and how many sandboxes it may run. It is namespaced, and it can only
narrow what its profile allows.

```yaml
apiVersion: sandbox.firestoned.io/v1alpha1
kind: SandboxPolicy
metadata:
  name: agent-default
  namespace: sandboxes
spec:
  profileRef: hardened
  subjects:
    groups: ["00000000-0000-0000-0000-000000000000"]
  audiences:
    - name: internal-git
      access: Read
      scopes: ["repo:read"]
  egress: []
  tools: []
```

This is `examples/sandboxpolicy-hardened.yaml`.

## Fields

| Field | Meaning | Checked by |
| --- | --- | --- |
| `profileRef` | A cluster-scoped SandboxProfile. | missing: `Ready=False, ProfileNotFound` |
| `subjects.groups` | Identity provider group ids allowed to claim a sandbox. Empty means nobody can. | `L010` (a warning) |
| `audiences[]` | Each downstream service the sandbox may get a token for: `name`, `access` (`Read` or `Write`), `scopes`, and an optional `ttlSeconds` that defaults to the K5.1 ceiling. | `L005` (fits K5.3, no wildcard), `L006` (write needs K5.2, TTL within K5.1) |
| `egress[]` | `host` and `port`, and at K4.1 `R2` or above, optional `methods` and `paths`. No IP literals. | `L007` |
| `tools[]` | A tool or MCP server by `name`, pinned by `manifestDigest` (`sha256:<hex>`). | `L008` (fits K1.3, pinned) |
| `lease` | `maxDurationSeconds` and `maxConcurrent`. Defaults to the K7.3 ceiling. | `L009` |
| `budgets` | `maxToolCalls`, `maxWallClockSeconds`, `maxEgressBytes`. Defaults from K7.3 and K4.3. | `L009` |

"Effective" knobs below means the profile's steps with every active [grant](grants.md) applied.

## Status

| Field | Meaning |
| --- | --- |
| `conditions[Ready]` | `True` with reason `Published` once a signed bundle is out. Otherwise `False` with the reason: see below. |
| `conditions[Valid]`, `conditions[ProfileResolved]` | Whether the policy linted clean and its profile resolved. |
| `bundleVersion`, `bundleDigest` | The published bundle. The version only moves when the content digest moves, and never goes backwards. |
| `bundleNotAfter` | When the published bundle stops being valid. |
| `profileDigest` | The digest of the profile it was built on. |
| `maxExposure` | What a fully compromised sandbox under this policy could reach, computed from the content (objective O3). Reported even when nothing is published, so approvers can see what they are being asked to approve. |
| `activeGrants`, `nextGrantExpiry` | The grants applied now, and the soonest one to lapse. |

## Why a policy is not Ready

| `Ready` reason | Meaning |
| --- | --- |
| `ProfileNotFound`, `InvalidProfile` | The profile is missing or fails lint. |
| `InvalidPolicy` | The policy fails a lint rule against its effective knobs. |
| `NoApprovedCeiling` | No [ExposureCeiling](ceilings.md) of the same name exists. |
| `CeilingNotApproved` | The ceiling lacks a `Security` approval. |
| `ExposureAboveCeiling` | The exposure, grants included, is wider than the ceiling. |
| `NotCanonical` | The content has no exact canonical form (ADR-0005), so it cannot be digested. |
| `NoSigner` | No signer is configured. Nothing is published unsigned. |
| `SigningFailed` | The signer refused or was unreachable. The previous bundle stays in place. |
| `FailOpenRefused` | The bundle failed a check the evaluator should already have made (ADR-0010). A bug: report it. |

Every change of reason is also recorded as a Kubernetes Event on the policy.

## What gets published

When the policy is Ready, notaio publishes a ConfigMap named `<policy>-bundle` in the policy's
namespace, owned by the policy, holding the signed [bundle](bundle.md) under `bundle.dsse.json`. Its
annotations `sandbox.firestoned.io/bundle-version` and `sandbox.firestoned.io/bundle-digest`
mirror the status, and are a second record of the version, so losing status cannot reset it.
