# ADR-0001: Notaio is a separate project

- Status: Accepted
- Date: 2026-09-30

## Context

The AgentSandbox design needs a declarative policy API (profiles, policies, knob grants) and
something that validates it, applies the threat model's rules and compiles it into a bundle that the
runtime components enforce. The question was where that lives: inside banlieue, inside mediatore, or
on its own.

## Decision

A separate repository with its own controller and minimal RBAC. Banlieue and mediatore are consumers
of its contracts: banlieue of the CRD types, mediatore and mediatore-guest of the signed bundle.

## Why not banlieue

Banlieue is a provider-agnostic VM API and is used for things that are not agents, such as the
OpenBao VMs. Token audiences, egress rules, tool allowlists and approval classes are AgentSandbox
concepts. Putting them in banlieue would make its API specific to one use case. Banlieue's
controller also holds hypervisor credentials, and the threat model says policy signing must not sit
with whoever holds the keys to the VMs (T8.2, scenario S6).

## Why not mediatore

Mediatore is the most trusted runtime component and its scope is already flagged as a risk. A
controller with Kubernetes API access and signing duties would enlarge its attack surface. Mediatore
consumes a signed, compiled bundle and never watches policy CRDs.

## Consequences

- One more repository and one more deployment on the management cluster.
- The contracts (types crate, bundle format, version rules) become the stable surface, and changing
  them is a cross-project change. They are kept in small crates for that reason.
- `sandboxpolicy-bundle` has no Kubernetes dependency so that mediatore-guest, which runs as root in
  every sandbox VM, does not inherit a cluster client.

## Related

AgentSandbox threat model: T5.4, T8.2, scenario S6, objectives O3 and O8, and the hard floor that
policy bundles are signed.
