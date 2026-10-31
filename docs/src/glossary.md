# Glossary

The identifiers and terms used across these docs. Where a family of codes has a full, current list,
it is generated from the code, and this page links to it rather than copying it.

## Identifiers

### `K1.1` to `K8.4`: knobs

A **knob** is one adjustable control from section 9 of the AgentSandbox threat model, such as
`K4.1` (egress mode) or `K5.1` (token lifetime). The first digit is the **trust boundary** it guards,
1 to 8; the second numbers the knob within it. There are 34.

| Boundary | Knobs | Guards |
| --- | --- | --- |
| 1 | `K1.1` to `K1.4` | content and tools that reach the agent, and output that leaves |
| 2 | `K2.1` to `K2.5` | command execution, filesystem, packages, resources, the command jail |
| 3 | `K3.1` to `K3.3` | credential delivery, approval mode, guard strictness |
| 4 | `K4.1` to `K4.5` | egress, DNS, volume caps, TLS inspection, inbound exposure |
| 5 | `K5.1` to `K5.6` | tokens: lifetime, scope, audiences, sender constraint, renewal, identity type |
| 6 | `K6.1` to `K6.3` | tenancy, virtual hardware, attestation |
| 7 | `K7.1` to `K7.4` | authentication, eligibility, lease limits, unattended runs |
| 8 | `K8.1` to `K8.4` | image source, policy change control, logging, break-glass |

Every knob's name, loosest step and approvers: [Knob Register](reference/knobs.md). What they mean
together: [Knobs and Steps](concepts/knobs.md).

### `R0` to `R3`: steps

The setting of a knob. `R0` is the locked default; each step above it is looser. Many knobs stop at
`R1` or `R2`. A profile sets a step per knob; a grant raises one knob by exactly one step.

### `L001` and up: lint rules

A **lint rule** is one check on profiles, policies, grants and ceilings, run by the controller and
by `notaio lint` alike, with a stable code that is never renumbered. They come in families:

| Codes | Check |
| --- | --- |
| `L001` to `L004` | a set of knob steps: known knobs, steps that exist, no toxic combination, Lab-only knobs on Lab only |
| `L005` to `L010` | a policy against its knobs: audiences, write access and token lifetime, direct egress, tools, lease and budgets, subjects |
| `L011` to `L013` | a grant: one step, a lifetime within its cap, every required approval |
| `L014` to `L016` | the exposure ceiling: the policy within it, a ceiling present, Security's approval |
| `L017`, `L018` | endpoints: well-formed and exact, reachable one way only |

The full, current list: [Lint Rules](reference/lint-rules.md).

### `I001` and up: input codes

An **input code** is a finding only `notaio lint` can report, about the set of files rather than
one object: an object that does not parse, the same object twice, a policy whose profile is
missing, a grant or ceiling whose policy is missing, an expired grant, content with no canonical
form, a profile that fails lint, a file at a retired API version. The full list:
[Lint Rules](reference/lint-rules.md#input-rules).

### `T5.4`, `S6`, `O3`, section 9: the threat model's identifiers

notaio implements part of the AgentSandbox threat model, and cites its identifiers:

| Form | Is a | Example |
| --- | --- | --- |
| `T<n>.<m>` | threat | `T8.2`, a silent edit of a policy; `T5.4`, a bundle downgrade or tampering |
| `S<n>` | scenario | `S6`, the separation of who holds the VM keys from who signs policy |
| `O<n>` | objective | `O3`, "maximum exposure is computable from its policy" |
| section 9, 10 | the knob register; the profile matrix | the source of `registry.rs` and `builtin.rs` |

What notaio implements, supports or leaves: [Threat Model Mapping](security/threat-model-mapping.md).

### Other numbers

| Form | Is | Where |
| --- | --- | --- |
| `ADR-0013` | an architecture decision record, never renumbered | [Decisions](adr/index.md) |
| `M0` to `M5` | a roadmap milestone | [Roadmap](roadmap.md) |
| roadmap `00` | a detailed plan for a milestone | `.github/community/` in the repository |
| `notaio:v1` | a signing key id: key name and version | [The Bundle](concepts/bundle.md#who-signs) |
| `v1alpha2` | the API version of the four kinds | [CRD API](reference/crds.md) |
| `bundle/v3` | the bundle schema | [The Bundle](concepts/bundle.md) |

## Terms

**AgentSandbox.** The platform notaio belongs to: banlieue runs the VMs, mediatore brokers identity
and tokens, mediatore-guest enforces inside each VM, and notaio says what each sandbox may do.

**Audience.** A downstream service a sandbox may get a token for, with an access level, a token
lifetime, and the endpoints and rules at which the token may be used.

**Brokered egress / direct egress.** A call to an audience endpoint goes through mediatore's proxy,
which injects the token: brokered, allowed at any `K4.1` step. Direct egress reaches a host without
the proxy or a token, and needs `K4.1` above `R0`.

**Bundle.** The signed, complete, effective policy notaio publishes for one SandboxPolicy, in a
ConfigMap `<policy>-bundle`. [The Bundle](concepts/bundle.md).

**Canonical form.** The exact bytes the content digest is taken over: RFC 8785 JSON, so any
language computes the same digest (ADR-0005).

**Conformance vectors.** Signed sample bundles, each with the verdict every consumer must reach, so
a consumer's verifier can be checked against notaio's own.
[How they work](concepts/bundle.md#how-a-consumer-proves-it-gets-this-right-the-conformance-vectors),
[the list](reference/conformance.md).

**Consumer.** Anything that verifies a bundle and enforces it: mediatore, mediatore-guest, the
gateway.

**DSSE envelope.** The signed wrapper around the bundle payload: the payload, its type, and
signatures over both.

**Endpoint, rule.** An endpoint is an exact `host:port`. A rule is `{ method, path }`, a method or
`*` and a path glob in which `*` is one segment and `**` any number. No rules means anything.

**Exposure, maximum exposure.** What a fully compromised sandbox under a policy could reach:
audiences and their endpoints, hosts, tools, write capability, lease. Computed into
`status.maxExposure`.

**ExposureCeiling.** The approved maximum exposure of one policy, approved by Security.
[ExposureCeiling](concepts/ceilings.md).

**Fail-open guard.** A second, independent check before signing, for a bundle the evaluator should
never have produced. It never fires in a correct build; `notaio_fail_open_total` counts it.

**Floor, version floor.** The highest bundle version a consumer has accepted per policy, kept
across restarts, so an older, looser bundle is refused (ADR-0008).

**Incarnation.** One life of a policy, identified by its `uid`. Deleting and recreating a policy
starts a new incarnation at version 1; the floor orders incarnations by `issuedAt`.

**KnobGrant, relaxation.** A time-boxed, approved, one-step loosening of one knob for one policy.
[KnobGrant](concepts/grants.md).

**Leader.** The one controller replica holding the Lease `notaio`, which alone reconciles and signs
(ADR-0011).

**`notAfter`, lifetime, refresh.** A bundle is valid until `notAfter`: one bundle lifetime (an hour
by default) after it is issued, never later than its earliest grant's expiry. The controller
re-signs it at half its lifetime.

**Profile.** A named set of knob steps; four are built in. [SandboxProfile](concepts/profiles.md).

**Ready reason.** Why a policy is or is not published, in its `Ready` condition:
[Troubleshooting](guides/troubleshooting.md).

**SBOM, provenance, VEX.** What ships with each image: the list of everything in it (SBOM), a signed
statement of how and from what it was built (SLSA provenance), and signed statements of which
scanner findings cannot affect it and why (OpenVEX). ADR-0009, ADR-0012.

**Transit key, key version, pin.** The signing key lives in OpenBao transit and never leaves it;
each rotation adds a version; `NOTAIO_OPENBAO_KEY_VERSION` pins the one notaio signs with.
[Rotate the Signing Key](guides/rotate-the-signing-key.md).
