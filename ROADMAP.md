# Roadmap

Ordered by consequence, like the AgentSandbox security roadmap this project belongs to. The
contracts (types, bundle, version rules) have to be stable before Roadmap A phase 1 of the threat
model finishes, because the attestation chain delivers a bundle.

## M0: scaffold and contracts (this repository state)

Done: types, CRDs, knob register and built-in profiles transcribed from the threat model, lint rules
L001 to L013, `evaluate`, bundle format with DSSE signing and verification, controller skeleton,
generated manifests, raw deployment manifests, ADR-0001 and ADR-0002, 39 tests.

Not done: nothing has run on a cluster.

## M1: make the contracts trustworthy

- Run the controller on a k0s test cluster end to end: CRDs, profiles, a policy, a grant, a bundle
  published and verified by a small consumer. Fix what breaks. Try the admission policy there too.
- `notaio lint` CLI that runs the same `sandboxpolicy-core` on files, for pull request checks. This
  is the threat model's "toxic-combination lint on every policy pull request".
- Approved-ceiling check: a way to record the approved maximum exposure of a profile or policy and
  fail a change that exceeds it (the "maximum exposure compared with its approved ceiling" test).
- Decide ADR-0002 open decisions 1, 2 and 4 (distribution, signing key custody, canonicalisation),
  then freeze bundle schema v1.
- Example verifier for mediatore and mediatore-guest, with persisted "highest version accepted".

## M2: production controller

- Leader election and more than one replica.
- Health endpoints, Prometheus metrics (relaxations by age and past expiry, lint failures, bundle
  age, fail-open counters that must be zero), Kubernetes Events.
- KMS, HSM or TPM backed `Signer` (ADR-0002, decision 2). Remove the development signer path from the
  default deployment.
- Image build, signing and provenance. Pin the deployment by digest.

## M3: integrate the consumers

- Banlieue: claims name a policy and profile, admission checks eligibility with these objects as
  parameters, the claim records the policy version. No policy logic in banlieue.
- Mediatore: verify the bundle, evaluate at every mint, stamp the version into tokens, refuse stale
  versions.
- Mediatore-guest: render the jail, nftables and managed settings from the verified bundle.
- Gateway: allowlist from the same bundle.

## M4: tighten and extend

- Tightening latency (ADR-0002, decision 3): version floor pushed to mediatore, shorter lifetime.
- Audience classification, so "Lab audiences only" becomes enforceable.
- K2.4 resource sizes once the model defines them.
- Cedar: compile a Cedar policy set into the bundle when the declarative form runs out.
- Knob dashboard over `KnobGrant`.

## M5: public edition

A separate, status-labelled edition of the docs (implemented, planned, proposed), a SECURITY.md, and
a licence review. Confirm employer sign-off first.
