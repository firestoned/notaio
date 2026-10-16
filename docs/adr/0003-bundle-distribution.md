# ADR-0003: Bundle distribution through a ConfigMap, relayed to guests by mediatore

- **Status:** Accepted
- **Date:** 2026-10-16
- **Deciders:** Erick Bourgeois
- **Resolves:** [ADR-0002](0002-bundle-contract.md) open decision 1 (distribution)
- **Related:** platform ADR-0081 and ADR-0082 (recorded in banlieue as ADR-0081 and ADR-0082, in
  mediatore as ADR-0009 and ADR-0010); mediatore ADR-0011 (claim watcher)

Not implemented. notaio already publishes as described in decision 1; the mediatore side is
ROADMAP M3.

## Context

ADR-0002 left open how a signed bundle reaches its consumers. v0 writes the envelope to a ConfigMap
next to the policy, and ADR-0002 noted that this "sits badly with mediatore never watches CRDs and
the guest never talks to a Kubernetes API server".

Looking at the consumers as they are built:

- **mediatore** runs in the management cluster and already reads the Kubernetes API: its claim
  watcher is a kube-rs watcher on `VirtualMachineClaim` (mediatore ADR-0011). What ADR-0001 rules
  out is mediatore watching the *policy objects* and so becoming a policy evaluator of raw
  specs. Reading a signed, compiled artefact is the opposite of that.
- **mediatore-guest** sits on the sandbox segment, which reaches only mediatore and the egress
  gateway, and never a Kubernetes API server (platform ADR-0082). Whatever the store, the guest's
  bundle has to arrive over the outbound mutual-TLS stream it already holds to mediatore.
- **The egress gateway** is not built yet, and where it runs is not decided.

Because every consumer verifies the signature, the expiry and the version floor (ADR-0002, decision
5), the transport does not need to be trusted for integrity. It only has to be available.

## Decision

1. **notaio publishes to a ConfigMap,** as v0 does: `<policy>-bundle` in the policy's namespace,
   the envelope under `bundle.dsse.json`, version and digest as annotations, owned by the
   policy. notaio stays the only writer. Its RBAC does not change.
2. **mediatore reads the bundles through the Kubernetes API** with read-only access to ConfigMaps
   in the namespaces that hold SandboxPolicy objects, watches them, verifies each new envelope and
   keeps the highest version it has accepted per policy.
3. **mediatore relays the envelope to each guest unchanged** over the existing mutual-TLS stream.
   The guest verifies it independently, with its own trusted keys and its own persisted version
   floor. The relay is transport, not a trust step: mediatore cannot alter a bundle without the
   guest refusing it.
4. **The gateway gets its bundle the same way as the other consumers** once its placement is
   decided: through the API if it runs in the management cluster, through a mediatore relay if it
   does not. That choice is recorded with the gateway.
5. **No consumer trusts the transport.** A bundle is accepted because it verifies, never because of
   where it was read from.

## Consequences

- notaio gains no credential, listener or network path. Its NetworkPolicy does not change.
- mediatore gains `get`, `list` and `watch` on ConfigMaps in the policy namespaces. RBAC cannot
  scope that to notaio's label, so mediatore can read every ConfigMap there. Policy namespaces
  should therefore hold only policy objects and their bundles, by convention and by review.
- Whoever can write those ConfigMaps can withhold a bundle or replay an older one. They cannot loosen
  a sandbox: a forged bundle fails the signature and an older one fails the version floor and
  expiry. Withholding is a denial of service, bounded by the bundle lifetime.
- Propagation to mediatore is a watch event, so it is near immediate; to guests it is a push on a
  stream they already hold. This does not resolve tightening latency for a consumer that is not
  connected (ADR-0002 open decision 3).
- A ConfigMap is capped at 1 MiB. A bundle is a few kilobytes; a policy large enough to approach
  the cap would already fail review.
- mediatore's platform CALM model (`agentsandbox-stack.calm.json`) has no bundle-to-guest
  relationship today. Add one, through mediatore, when this is implemented.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
