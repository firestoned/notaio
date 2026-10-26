# Knobs and steps

A **knob** is one adjustable control from section 9 of the AgentSandbox threat model, such as
"Egress mode" or "Token TTL". There are 34 of them, grouped by the trust boundary they guard, and
each has an id like `K4.1`. The register lives in
`crates/sandboxpolicy-core/src/registry.rs`, transcribed from the threat model, which is the
source of truth.

## Steps

Every knob is set to a **step**:

| Step | Meaning |
| --- | --- |
| `R0` | The locked default. A knob that is not mentioned is at `R0`. |
| `R1`, `R2`, `R3` | Progressively looser. Each knob has a loosest step it allows; many stop at `R1` or `R2`. |

A step above the knob's loosest is refused (lint `L002`), and an unknown knob id is refused
(`L001`).

## The eight boundaries

| Boundary | Knobs | Guards |
| --- | --- | --- |
| 1 | K1.1 to K1.4 | What content and tools reach the agent, and what leaves as output |
| 2 | K2.1 to K2.5 | Command execution, filesystem, packages, resources, the command jail |
| 3 | K3.1 to K3.3 | Credential delivery, approval mode, guard strictness |
| 4 | K4.1 to K4.5 | Egress, DNS, volume caps, TLS inspection, inbound exposure |
| 5 | K5.1 to K5.6 | Tokens: lifetime, scope, audiences, sender constraint, renewal, identity type |
| 6 | K6.1 to K6.3 | Tenancy, virtual hardware, attestation |
| 7 | K7.1 to K7.4 | Authentication, eligibility, lease limits, unattended runs |
| 8 | K8.1 to K8.4 | Image source, policy change control, logging, break-glass |

The full register, with each knob's name, loosest step and approvers, is in the
[Knob Register](../reference/knobs.md).

## Approvers

Each knob names the roles that must approve a relaxation of it: `Security`, `PlatformOwner`,
`DataOwner`, `Compliance`, `ToolOwner` or `RiskOwner`. A [KnobGrant](grants.md) must carry an
approval from **every** role its knob names (`L013`). Relaxing `K7.4` (unattended runs), for
example, takes the platform owner, Security and the risk owner.

## Rules that span knobs

Some knobs are safe alone and dangerous together. notaio checks these on the effective knob set,
grants included, every time:

- **No toxic combination (`L003`).** Sensitive data, untrusted content and an outbound channel must
  never all be reachable at once:
    - *sensitive data*: `K2.2` or `K5.3` above `R0`, on a profile whose `dataScope` is `Internal`;
    - *untrusted content*: `K1.1` at `R2` or above;
    - *outbound channel*: `K4.1` at `R2` or above, or `K5.2` at `R1` or above (any write scope).

    A `LabOnly` profile holds no sensitive data by construction, so the first leg never applies
    to it. That is what lets the Lab profile combine open fetching with open egress.
- **Lab-only knobs (`L004`).** `K6.3` (attestation strictness) and `K8.2` (policy change control)
  may be relaxed only on a `LabOnly` profile.

## Ceilings attached to steps

Some steps carry numeric limits. They are defaults "to tune, not commitments" in the threat model,
kept as data in `crates/sandboxpolicy-core/src/ceilings.rs`:

| Knob | `R0` | `R1` | `R2` |
| --- | --- | --- | --- |
| K5.1 token TTL | 5 minutes | 15 minutes | 1 hour |
| K4.3 egress per lease | 10 MiB | 100 MiB | 1 GiB |
| K7.3 lease duration | 4 hours | 8 hours | 24 hours |
| K7.3 concurrent sandboxes per user | 1 | 2 | 5 |

A policy that asks for more than its effective step allows is refused (`L006`, `L009`). When a
policy leaves a lease or budget out, it gets the ceiling for its step, and a tool-call budget of
500.
