# Write a policy

This takes a team from nothing to a published, signed bundle: pick a profile, write the policy and
the ceiling it needs, check both, and merge.

## 1. Pick a profile

Start from the tightest profile that does the job; a grant can loosen one knob later.

| If the sandbox | Profile |
| --- | --- |
| reads sensitive material and must have no outbound path at all | `fortress` |
| works on code and documents against internal systems, read-only | `hardened` |
| does everyday engineering work: builds, tests, limited egress | `standard` |
| runs experiments on non-production data only | `lab` |

The step of every knob in each is in [Built-in Profiles](../reference/profiles.md). What the steps
mean is in [Knobs and steps](../concepts/knobs.md).

## 2. Write the policy

```yaml
apiVersion: sandbox.firestoned.io/v1alpha1
kind: SandboxPolicy
metadata:
  name: docs-agent
  namespace: team-docs
spec:
  profileRef: hardened
  subjects:
    groups: ["<identity provider group id>"]
  audiences:
    - name: internal-git
      access: Read
      scopes: ["repo:read"]
  egress: []
  tools: []
```

Things that are refused, and why:

- **More audiences, or write access, than the profile's K5.3 and K5.2 steps allow** (`L005`,
  `L006`). Hardened is K5.3 `R1`, K5.2 `R0`: one audience, read-only.
- **A wildcard audience, or a token longer than the K5.1 ceiling** (`L005`, `L006`). Leave
  `ttlSeconds` out to get the ceiling: 5 minutes at `R0`.
- **Egress the K4.1 step does not allow** (`L007`). Hardened is `R0`: no entries, every call is
  brokered. Never an IP literal.
- **A tool not pinned by `sha256:` manifest digest** (`L008`).
- **A lease or budget above the K7.3 and K4.3 ceilings** (`L009`). Leave them out for the ceiling.

An empty `subjects.groups` is allowed but warned about (`L010`): nobody could claim a sandbox.

## 3. Write its ceiling

A policy publishes nothing without an [ExposureCeiling](../concepts/ceilings.md) of the same name.
The ceiling lists what the policy may reach, and a person in the `Security` role approves it:

```yaml
apiVersion: sandbox.firestoned.io/v1alpha1
kind: ExposureCeiling
metadata:
  name: docs-agent
  namespace: team-docs
spec:
  audiences:
    - name: internal-git
      access: Read
      maxTtlSeconds: 300
  egressHosts: []
  tools: []
  maxLeaseSeconds: 14400
  approvals:
    - role: Security
      reference: <link to this pull request>
```

If you are not sure what to put in it, start with an empty ceiling (`audiences: []`,
`egressHosts: []`, `tools: []`, `maxLeaseSeconds: 0`) and run `notaio lint`: each `L014` finding
names one audience, host, tool or lease value the policy reaches that the ceiling does not cover.
Once merged, the policy's `status.maxExposure` shows the same, computed in the cluster. See
[Approve an exposure ceiling](approve-a-ceiling.md).

## 4. Check it before you open the pull request

```sh
notaio lint team-docs/
```

It runs the controller's own rules, ceilings included, on the files, and exits 1 on any error
finding. Each finding names its code and the field. In a policy repository with
[CI linting](lint-in-ci.md), the same findings appear inline on the pull request.

The namespace needs the `notaio-bundles` Role and RoleBinding, or the controller cannot publish
there ([Install](install.md#5-allow-bundle-writes-and-lock-down-direct-edits)).

## 5. Merge, and watch it publish

```sh
kubectl -n team-docs get sandboxpolicy docs-agent
# NAME         PROFILE    VERSION   READY
# docs-agent   hardened   1         True

kubectl -n team-docs get configmap docs-agent-bundle
kubectl -n team-docs get events --field-selector reason=Published
```

`Ready=True` with reason `Published` means a signed bundle is in `docs-agent-bundle`, and the
consumers will pick it up. If it is not Ready, the reason says why: see
[Troubleshooting](troubleshooting.md).

## Changing it later

Edit and merge again. The bundle version moves only if the compiled content changes. A change
that widens what the policy reaches also needs its ceiling widened, in the same pull request or
first, and that needs Security again.
