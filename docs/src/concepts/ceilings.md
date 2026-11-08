---
status: implemented
---

# ExposureCeiling

An **ExposureCeiling** is the approved maximum of what one policy may expose: the audiences it may
get tokens for, the hosts it may call, its tools and its longest lease. It has the same name and
namespace as its [SandboxPolicy](policies.md). It exists because lint rules check each part of a
policy against its knobs, but no rule can say "this team should reach exactly these systems". A
person in the `Security` role says that, once, here ([ADR-0007](../adr/0007-exposure-ceiling.md)).

```yaml
apiVersion: sandbox.firestoned.io/v1alpha2
kind: ExposureCeiling
metadata:
  name: agent-default        # the policy's name
  namespace: sandboxes
spec:
  audiences:
    - name: internal-git
      access: Read           # Write would allow Read too
      maxTtlSeconds: 300
      endpoints: ["git.foo.io:443"]   # where its token may be used
      dataScope: Internal    # LabOnly lets a Lab policy expose it; unset means Internal
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
| An audience, an audience endpoint, a host or a tool the ceiling does not list, more access or a longer token than it allows, or a longer lease | `L014` | `ExposureAboveCeiling` |
| A policy on a `LabOnly` profile exposes an audience the ceiling does not classify `LabOnly` | `L020` | `ExposureAboveCeiling` |

In each case nothing new is published, and `maxExposure` is still reported so the approvers can see
exactly what they are being asked to approve. The same check runs in `notaio lint` on the policy
repository's pull requests, so a change that would not publish fails review first.

## Lab audiences

The Lab profile may combine untrusted content with an open outbound channel because it holds no
sensitive data ([knobs](knobs.md#rules-that-span-knobs)). Its audiences are where that could stop
being true, so each ceiling audience says what data it serves: `dataScope: LabOnly` or `Internal`
([ADR-0017](../adr/0017-audience-classification.md)). A policy on a `LabOnly` profile may expose
only `LabOnly` audiences (`L020`); policies on other profiles may expose either.

An audience without `dataScope` is `Internal`. Calling an audience lab loosens what a Lab policy may
do with it, so it is the ceiling's approver, Security, who says so, never the policy's author.

Each ceiling classifies its own audiences, so two ceilings can disagree about the same one: lab in
one, internal (or unset) in the other. One of them is then wrong. `notaio lint` warns on both
(`L021`) when it sees both, which on a policy repository is before Security signs either. It is a
warning, not an error, because each policy is still checked against its own ceiling; the
controller does not report it.

Before upgrading to a notaio with `L020`, classify the audiences of every ceiling whose policy uses
a `LabOnly` profile; `notaio lint` on the policy repository lists any that are missing. Otherwise
those policies stop publishing on the first reconcile, and their sandboxes can mint nothing once the
last bundle expires.

## Why it is a separate object

Widening what a policy can reach takes two approvals: the policy change, and the ceiling change.
Keep ceilings under a path in the policy repository that CODEOWNERS gives to the security
approvers. The controller only reads ceilings; it has no write access to them, and the GitOps-only
admission policy covers them like the other kinds.
