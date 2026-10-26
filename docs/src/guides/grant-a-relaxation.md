# Grant a relaxation

A [KnobGrant](../concepts/grants.md) loosens one knob, by one step, for one policy, until a date.
Use it when a team needs a little more than its profile gives, for a reason that will end.

## 1. Find the knob and the step

Look up the knob in [Knobs and steps](../concepts/knobs.md) and the profile's current step for it
in [Built-in Profiles](../reference/profiles.md). The grant's step must be **exactly one above**
the profile's (`L011`). If you need two steps, you need a different profile.

## 2. Find the approvers and the longest expiry

The register names the roles that must approve each knob (in
[`registry.rs`](https://github.com/firestoned/notaio/blob/main/crates/sandboxpolicy-core/src/registry.rs)),
and the target step caps how far away `expiresAt` may be:

| Target step | Longest expiry from now |
| --- | --- |
| `R1` | 90 days |
| `R2` | 30 days |
| `R3` | 7 days |

## 3. Write it

```yaml
apiVersion: sandbox.firestoned.io/v1alpha1
kind: KnobGrant
metadata:
  name: docs-agent-pkg-mirror
  namespace: team-docs
spec:
  policyRef: docs-agent
  knob: K2.3                       # package installation; hardened has R0
  step: R1
  owner: team-docs
  reason: The build needs the internal package mirror until the toolchain image ships.
  expiresAt: "2026-12-01T00:00:00Z"
  approvals:
    - role: PlatformOwner          # every role the register names for K2.3
      reference: <link to this pull request>
```

Write the `reason` for the person reading the relaxation dashboard in two months.

## 4. Check, review, merge

```sh
notaio lint team-docs/
```

Besides `L011` to `L013`, the grant is checked together with everything else on the policy: it is
refused if it would complete a [toxic combination](../concepts/knobs.md#rules-that-span-knobs), and
the policy is refused if the grant widens its exposure past its ceiling. In that case widen the
ceiling in the same pull request, with Security's approval.

The approvers approve the pull request; their approvals are recorded in the grant, each with a
reference to the reviewed change.

## 5. Watch it apply

```sh
kubectl -n team-docs get knobgrant docs-agent-pkg-mirror
# NAME                    POLICY       KNOB   STEP   PHASE    EXPIRES
# docs-agent-pkg-mirror   docs-agent   K2.3   R1     Active   2026-12-01T00:00:00Z
```

`Active` means the next bundle carries it, with `notAfter` no later than the grant's expiry.
`Rejected` means it broke a rule; `kubectl describe` shows the finding and its code, and a
`Rejected` Event is on the grant.

## When it ends

On its own. At `expiresAt` the bundle carrying it expires, the grant becomes `Expired`, and a new
bundle without it is published. To keep it, move `expiresAt` out through another reviewed change
before then; the cap is measured from the time of evaluation, so the new date must still be within
it. Delete the grant when it is no longer needed, so the dashboard stays honest.
