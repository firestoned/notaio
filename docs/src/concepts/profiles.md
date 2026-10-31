# SandboxProfile

A **SandboxProfile** is a named set of knob steps. It is cluster scoped and platform owned: teams
pick a profile for their policy, they do not edit one.

```yaml
apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxProfile
metadata:
  name: hardened
spec:
  description: Code and document work against internal systems, read-only
  dataScope: Internal     # or LabOnly
  knobs:
    K1.2: R1
    K1.3: R1
    K5.3: R1
    # ... every knob not listed stays at R0
```

| Field | Meaning |
| --- | --- |
| `spec.dataScope` | `Internal`, or `LabOnly` for a profile that by construction holds no sensitive data. It decides whether the toxic-combination rule's first leg applies ([knobs](knobs.md#rules-that-span-knobs)). |
| `spec.knobs` | Step per knob id. Absent means `R0`. |
| `status.conditions[Valid]` | `True` when the profile passes `L001` to `L004`. |
| `status.digest` | A digest of the validated spec, set only while valid. Every bundle records it, so a profile change is visible downstream. |

## The built-in profiles

notaio ships the four profiles of the threat model's profile matrix (section 10), generated from
`crates/sandboxpolicy-core/src/builtin.rs` into `deploy/profiles/builtin.yaml`:

| Profile | For | Data scope |
| --- | --- | --- |
| `fortress` | Analysis of sensitive material with no outbound path. Every knob at `R0`. | Internal |
| `hardened` | Code and document work against internal systems, read-only | Internal |
| `standard` | Everyday engineering work | Internal |
| `lab` | Experiments and tool evaluation on non-production data | LabOnly |

The step of every knob in every profile is in the [built-in profiles reference](../reference/profiles.md).
A test, `registry_matches_profiles`, fails if a knob in the register is missing from any built-in
profile, so the matrix cannot silently fall behind the threat model.

## What the controller does with one

The profile reconciler lints the profile and writes `Valid` and the digest. A profile change
re-evaluates every policy that references it, so tightening a profile tightens every bundle built
on it at the next reconcile. An invalid profile publishes nothing for any policy that uses it
(`Ready=False, InvalidProfile`), and the controller records a warning Event on the profile.
