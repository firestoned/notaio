# KnobGrant

A **KnobGrant** is a time-boxed relaxation of one knob for one policy. It is how a team gets a
little more than its profile allows, without a new profile and without a permanent change. Every
relaxation has an owner, a reason, the approvals the register requires, and an expiry, so "which
relaxations exist, why, and until when" is a query over this one kind.

```yaml
apiVersion: sandbox.firestoned.io/v1alpha2
kind: KnobGrant
metadata:
  name: agent-default-pkg-mirror
  namespace: sandboxes
spec:
  policyRef: agent-default
  knob: K2.3          # package installation
  step: R1            # the hardened profile has R0
  owner: team-algo-platform
  reason: Build toolchain needs the internal package mirror.
  expiresAt: "2026-10-30T00:00:00Z"
  approvals:
    - role: PlatformOwner
      reference: https://github.com/firestoned/notaio/pull/1
```

This is `examples/knobgrant-package-install.yaml`.

## The rules a grant must meet

| Rule | Code |
| --- | --- |
| The target step is exactly one above the profile's step for that knob. No jumps. | `L011` |
| At most one active grant per knob per policy. | `L011` |
| `expiresAt` is within the cap for the target step: `R1` 90 days, `R2` 30 days, `R3` 7 days. | `L012` |
| An approval from every role the register names for the knob, each with a `reference` to the reviewed change. | `L013` |
| With the grant applied, the knob set still passes `L001` to `L004`: a grant cannot complete a toxic combination. | `L002` to `L004` |

## Phases

| `status.phase` | Meaning |
| --- | --- |
| `Pending` | Not evaluated yet, for example because its policy does not exist. |
| `Active` | Applied to the policy's effective knobs, and to its bundle. |
| `Rejected` | It breaks a rule above. The reason, with codes, is in its `Applied` condition. Other grants are unaffected. |
| `Expired` | `expiresAt` has passed. It is ignored and reported. |

Grants are applied in name order, each checked alone and then with the knob set as it stands, so
the result is deterministic and one bad grant does not disturb the others.

## How a relaxation ends

There is no revocation step. A bundle's `notAfter` is clamped to the earliest expiry of the grants
it applies, so when a grant lapses, the bundle carrying it lapses at the same moment and consumers
refuse it. The controller schedules a reconcile for that instant and publishes a bundle without
the grant. `notaio_relaxations_past_expiry` counts active grants past their expiry and must stay at
0; a lapse is also recorded as an `Expired` Event on the grant.

A grant can widen what the policy exposes. If it does, the policy's
[ExposureCeiling](ceilings.md) must cover the wider exposure too, or nothing is published.
