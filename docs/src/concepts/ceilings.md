# ExposureCeiling

An **ExposureCeiling** is the approved maximum of what one policy may expose: the audiences it may
get tokens for, the hosts it may call, its tools and its longest lease. It has the same name and
namespace as its [SandboxPolicy](policies.md). It exists because lint rules check each part of a
policy against its knobs, but no rule can say "this team should reach exactly these systems". A
person in the `Security` role says that, once, here ([ADR-0007](../adr/0007-exposure-ceiling.md)).

```yaml
apiVersion: sandbox.firestoned.io/v1alpha1
kind: ExposureCeiling
metadata:
  name: agent-default        # the policy's name
  namespace: sandboxes
spec:
  audiences:
    - name: internal-git
      access: Read           # Write would allow Read too
      maxTtlSeconds: 300
  egressHosts: []            # host:port
  tools: []
  maxLeaseSeconds: 14400
  approvals:
    - role: Security
      reference: https://github.com/firestoned/notaio/pull/1
```

This is `examples/exposureceiling-agent-default.yaml`.

## How it is checked

notaio computes the policy's `status.maxExposure` from the compiled content, active grants
included, and compares it with the ceiling:

| Finding | Code | `Ready` reason |
| --- | --- | --- |
| No ceiling with the policy's name | `L015` | `NoApprovedCeiling` |
| The ceiling has no `Security` approval | `L016` | `CeilingNotApproved` |
| An audience, host or tool the ceiling does not list, more access or a longer token than it allows, or a longer lease | `L014` | `ExposureAboveCeiling` |

In each case nothing new is published, and `maxExposure` is still reported so the approvers can see
exactly what they are being asked to approve. The same check runs in `notaio lint` on the policy
repository's pull requests, so a change that would not publish fails review first.

## Why it is a separate object

Widening what a policy can reach takes two approvals: the policy change, and the ceiling change.
Keep ceilings under a path in the policy repository that CODEOWNERS gives to the security
approvers. The controller only reads ceilings; it has no write access to them, and the GitOps-only
admission policy covers them like the other kinds.
