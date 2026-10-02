# Architecture

## Purpose

The AgentSandbox threat model says access is added by turning named knobs, each with a locked
default, a prerequisite, an approver and an expiry. This project makes that model executable: it is
where a knob setting is checked, where the exfiltration triangle is ruled out, and where a policy
becomes a signed artifact the rest of the platform can trust.

## The three kinds

**SandboxProfile** (cluster scoped). A named set of knob steps, `R0` to `R3`, plus a `dataScope`.
`dataScope: LabOnly` records that the profile holds no sensitive data, which is what lets Lab combine
untrusted content with an open outbound channel. Missing knobs mean `R0`. The four built-in profiles
are in `deploy/profiles/builtin.yaml`, generated from `sandboxpolicy-core/src/builtin.rs`.

**SandboxPolicy** (namespaced). Names a profile and adds the concrete audiences (with `Read` or
`Write` access and token TTL), egress allowlist entries, tools pinned by manifest digest, and
optionally lease and budget limits. It can only narrow what its profile allows.

**KnobGrant** (namespaced). A relaxation of one knob for one policy. It must be exactly one step
above the profile, carry an owner, a reason, an expiry inside the cap for its step (R1 90 days, R2 30,
R3 7) and an approval from every role the register names for that knob, each with a reference to the
reviewed change.

## Evaluation

`sandboxpolicy_core::evaluate` is a pure function of the policy, its profile, its grants and `now`.

1. The profile must exist and lint clean (L001 to L004). Otherwise nothing compiles.
2. Grants are applied in name order. Each is checked alone (L011 to L013), then the knob set with the
   grant applied is linted again, so a grant that would complete the exfiltration triangle is
   rejected without disturbing the others. Expired grants are reported and ignored.
3. The policy is linted against the effective knobs (L005 to L010).
4. The content is compiled with every list sorted, digested, and versioned: the version moves only
   when the digest moves.
5. Bundle validity is `now + lifetime`, clamped to the earliest active grant expiry, so a relaxation
   lapses on its own with no revocation mechanism.
6. `status.maxExposure` records what a fully compromised sandbox could reach (objective O3).

The lint rules and their codes are documented at the top of
`crates/sandboxpolicy-core/src/lint.rs`.

## The controller

Two reconcilers in one process. The profile reconciler lints and records a digest. The policy
reconciler gathers the profile and grants, calls `evaluate`, updates grant statuses, signs and
publishes the bundle to a ConfigMap named `<policy>-bundle` in the policy's namespace, and patches
status.

- **Idempotent.** It writes only when something other than the refresh timestamp changed, so its own
  status writes do not cause a reconcile loop.
- **Fails closed.** No profile, an invalid profile or an invalid policy means no new bundle. With no
  signing key configured it validates and reports `Ready=False, NoSigner` and publishes nothing.
  Unsigned bundles are never published.
- **Monotonic version.** The previous version is the higher of the status and the published
  ConfigMap annotation, so losing status cannot reset it.
- **Least privilege.** Read on the three kinds, `patch` on their status, `get/create/patch` on
  ConfigMaps in named namespaces only. It cannot create, edit or delete a policy object, and it holds
  no hypervisor credentials. It takes no inbound connections.

## The bundle

Defined in `sandboxpolicy-bundle`, described in [ADR-0002](adr/0002-bundle-contract.md). A consumer
verifies the DSSE signature, the payload type, the schema, the content digest, the validity window
and that the version is not below the highest it has accepted. Every failure is a refusal.

## What this project does not do

- It does not evaluate policy at mint time. Mediatore does, using the bundle.
- It does not create VMs or hold hypervisor credentials. Banlieue does.
- It does not deliver instructions to agents (`AgentTask`). That is a separate controller, still
  proposed in the threat model's ADR-0002.
- It does not enforce lease, budget, egress or tool limits. It compiles them, and the consumers
  enforce them.
- It does not check profile eligibility by group (K7.2). That is banlieue admission, using these
  objects as parameters.

## Known gaps in v0

- The controller's I/O path, the admission policy and the deployment manifests have not run on a
  cluster.
- The ConfigMap is a v0 distribution mechanism. Whether consumers should read it from the Kubernetes
  API at all is open decision 1 in ADR-0002.
- The development signer keeps a key in a Secret. The threat model requires the signing key not to be
  held by cluster administrators (T8.2). A KMS, HSM or TPM backed signer is open decision 2.
- A policy that becomes invalid leaves the previous bundle in place until it expires. The bundle
  lifetime is therefore the upper bound on how long a tightening takes to land. Faster tightening is
  open decision 3.
- K5.2 and K5.3 "apply to lab audiences only" for Lab cannot be enforced without a classification of
  audiences. `dataScope` covers the triangle, not this.
- K2.4 (resource limits) is not checked: the model does not define the medium and large sizes.
