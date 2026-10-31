# Architecture

## Purpose

The AgentSandbox threat model says access is added by turning named knobs, each with a locked
default, a prerequisite, an approver and an expiry. This project makes that model executable: it is
where a knob setting is checked, where the exfiltration triangle is ruled out, and where a policy
becomes a signed artifact the rest of the platform can trust.

## Where notaio sits in AgentSandbox

AgentSandbox runs agent-generated code on a person's behalf, with that person's credentials, and
assumes the agent will eventually be steered by hostile content. The platform is therefore split
into small components that each own one boundary, and notaio owns the one that says what a sandbox
may do. It does not run sandboxes, mint tokens or filter traffic. It decides, signs, and leaves
enforcement to the components that sit on each path.

```mermaid
flowchart TB
    subgraph git["Git: reviewed changes only"]
        policy["SandboxProfile · SandboxPolicy · KnobGrant"]
    end

    subgraph mgmt["Management cluster: orchestration only"]
        direction LR
        gitops["GitOps reconciler<br/>the only writer,<br/>enforced by admission"]
        notaio["<b>notaio</b><br/>lint, compile, sign"]
        bundle[("signed bundle<br/>DSSE, versioned, expiring")]
        banlieue["banlieue<br/>pools and claims,<br/>one VM per identity"]
        mediatore["mediatore<br/>login, binding,<br/>short-lived tokens"]
        gitops --> notaio --> bundle
    end

    user(["person"])

    subgraph vm["Sandbox VM: outside any cluster, own vTPM, no inbound"]
        direction LR
        agent["agent (jailed)<br/>empty network namespace"]
        guest["mediatore-guest (root)<br/>jail, nftables, settings"]
        agent -- socket --> guest
    end

    gateway["egress gateway<br/>allowlist"]
    world(["allowed destinations"])

    policy --> gitops
    user -- login --> mediatore
    banlieue -- "creates, outside the cluster" --> vm
    guest == "outbound mTLS, the only way out" ==> mediatore
    guest -- "jailed traffic" --> gateway --> world

    bundle -. verified by .-> mediatore
    bundle -. verified by .-> guest
    bundle -. allowlist .-> gateway
```

Solid arrows are the only paths that exist. Dotted arrows are where the signed bundle is enforced.
Sandboxes take no inbound connections and have no route to any Kubernetes API server
([banlieue ADR-0081](https://github.com/firestoned/banlieue/blob/main/docs/adr/0081-kubernetes-orchestrates-vms-run-agents.md),
[ADR-0082](https://github.com/firestoned/banlieue/blob/main/docs/adr/0082-no-inbound-to-sandboxes.md)).

### The components

| Component | Runs | Relationship to notaio | Today |
| --- | --- | --- | --- |
| **notaio** | management cluster; policy repository CI | The only producer of bundles. `notaio controller` watches the four kinds, writes their status and the bundle, and nothing else. `notaio lint` runs the same checks on the policy repository's files in every pull request ([ADR-0006](adr/0006-notaio-lint-cli.md)). | implemented; e2e on kind and k0s |
| Policy repository | Git | Where the four kinds are authored and reviewed; ceilings sit under a path its CODEOWNERS gives to the security approvers. A `KnobGrant`'s approvals reference the reviewed change. | by convention |
| GitOps reconciler | management cluster | The only identity allowed to create, change or delete the four kinds (`deploy/admission/gitops-only.yaml`). notaio itself cannot (`deploy/controller/10-rbac.yaml`). | implemented; `e2e_admission`, `e2e_rbac` |
| [banlieue](https://github.com/firestoned/banlieue) | management cluster | Creates one VM per identity, outside any cluster, and hands it out through a claim. Consumes the CRD types (`sandboxpolicy-api`): its admission checks claim eligibility with these objects as parameters, and the claim records the policy version. No policy logic lives in banlieue. | M3 |
| [mediatore](https://github.com/firestoned/mediatore) | management cluster | Binds a person's login to an attested VM and mints short-lived, one-audience tokens. Verifies the bundle, evaluates it at every mint, stamps the bundle version into the token and refuses a stale one. Never watches policy objects. | M3 |
| [mediatore-guest](https://github.com/firestoned/mediatore/tree/main/crates/mediatore-guest) | root, inside every sandbox VM | Renders the jail, the nftables ruleset and the agent's settings from the verified bundle. Depends on `sandboxpolicy-bundle` only, which has no Kubernetes dependency, so a root process in a hostile VM inherits no cluster client. Never evaluates policy for itself. | M3 |
| Egress gateway | sandbox network edge | The only place the jailed agent's traffic can go. Takes its allowlist from the same bundle. | M3 |

### The contracts

Everything that crosses a component boundary is one of these. Changing one is a cross-project
change, which is why they sit in small crates with their own ADR.

| Contract | Written by | Read by | Defined in | State |
| --- | --- | --- | --- | --- |
| `SandboxProfile`, `SandboxPolicy`, `KnobGrant` (`sandbox.firestoned.io/v1alpha1`) | GitOps reconciler (spec), notaio (status only) | notaio; banlieue admission (M3) | `crates/sandboxpolicy-api`, `deploy/crds/` | v1alpha1 |
| Signed bundle: a DSSE envelope over the compiled content, canonicalised with RFC 8785 | notaio, and only notaio | mediatore, mediatore-guest, gateway | `crates/sandboxpolicy-bundle`, [ADR-0002](adr/0002-bundle-contract.md), [ADR-0005](adr/0005-canonical-form-rfc-8785.md) | implemented; schema v2, frozen |
| Bundle location | notaio | mediatore through the API; guests through mediatore's relay | ConfigMap `<policy>-bundle` beside the policy, with version and digest annotations ([ADR-0003](adr/0003-bundle-distribution.md)) | published today; the relay is M3 |
| Consumer version floor: the highest version accepted per policy, keyed by namespace and name, with the incarnation's `uid` | each consumer, on its own disk | that consumer | `crates/sandboxpolicy-bundle/src/floor.rs`, [ADR-0008](adr/0008-consumer-version-floor.md) | implemented; consumers adopt it in M3 |
| Signing key and its public half | OpenBao transit, non-exportable; notaio may only request signatures | every consumer, configured out of band, key id `<name>:v<N>` | [ADR-0004](adr/0004-signing-key-custody.md), `deploy/signer/` | implemented, and the default |
| Policy status: `Ready`, version, digest, `maxExposure`, active grants | notaio | operators, reviewers, dashboards | `crates/sandboxpolicy-api` | implemented |

### The life of a change

```mermaid
sequenceDiagram
    autonumber
    actor Author
    participant Git as Policy repository
    participant GitOps as GitOps reconciler
    participant API as Kubernetes API<br/>(gitops-only admission)
    participant N as notaio
    participant CM as Bundle ConfigMap
    participant C as Consumers<br/>(mediatore, mediatore-guest, gateway)

    Author->>Git: pull request: policy, profile or KnobGrant
    Note over Git: review, and the approvals the knob register names
    Git->>GitOps: merge
    GitOps->>API: apply
    API-->>GitOps: admitted (any other writer is refused)
    API-->>N: watch event
    N->>N: lint, apply grants, compile, sign
    N->>API: status: Ready, version, digest, maxExposure
    N->>CM: signed envelope, version annotation
    C->>CM: mediatore watches (ADR-0003), relays to guests
    C->>C: verify signature, expiry, version floor
    Note over C: enforce, or refuse and keep the last good bundle
```

The bundle version moves only when the compiled content changes, so a consumer can treat "newer
version" as "different rules". Its `notAfter` is never later than the earliest active grant expiry,
so a relaxation ends on its own with nothing to revoke. A tightening, or a deleted policy, takes
effect only as fast as the bundle lifetime for a consumer that does not re-fetch (ADR-0002 open
decision 3).

### The life of a sandbox

How the pieces meet at runtime once the consumers are wired up. Steps marked M3 are the planned
consumer integration (`ROADMAPS.md`); the rest exists in banlieue and mediatore today.

```mermaid
sequenceDiagram
    autonumber
    actor P as Person
    participant M as mediatore
    participant B as banlieue
    participant G as mediatore-guest<br/>(root, in the VM)
    participant A as agent<br/>(jailed)
    participant GW as egress gateway

    P->>M: login
    M->>B: VirtualMachineClaim for this person
    Note over B: M3: admission checks eligibility<br/>against the SandboxPolicy
    B-->>M: bound to a warm, attested VM
    G->>M: outbound mTLS (the VM's only connection)
    Note over G: M3: render jail, nftables,<br/>settings from the verified bundle
    A->>G: request a token (local socket)
    G->>M: forward
    Note over M: M3: evaluate the bundle,<br/>stamp its version into the token
    M-->>G: short-lived, one-audience token
    G-->>A: token
    A->>G: outbound traffic (local socket)
    G->>GW: forward, to the gateway only
    Note over GW: M3: allowlist from the same bundle
```

### Who trusts what

- **Consumers trust the key, not the messenger.** A consumer accepts a bundle because it verifies
  against a trusted public key, never because of where it was read from. Anything that can write the
  bundle ConfigMap can withhold a bundle or replay an older one. It cannot loosen a sandbox: a forged
  bundle fails the signature, and an older one fails the version floor and expiry. The floor is
  persisted before a bundle is acted on, so a restart does not reopen the downgrade; a recreated
  policy is accepted because its bundle was issued later, and a replay from the old incarnation is
  refused ([ADR-0008](adr/0008-consumer-version-floor.md)).
- **notaio trusts only the API server.** It takes no inbound connection that can influence what it
  signs: its one listener serves health and metrics, reads nothing from a request but its path, and
  admits only the monitoring namespace ([ADR-0010](adr/0010-health-metrics-and-events.md)). It
  holds no hypervisor credentials, and no agent traffic reaches it. Its RBAC is read on the four
  kinds, status writes, Events, its own leader Lease, and bundle writes in named namespaces.
- **Duties are split on purpose.** banlieue holds the keys to the VMs, mediatore holds the power to
  mint tokens, and notaio holds the power to sign policy. The threat model requires that whoever
  holds the keys to the VMs does not also sign the policy (T8.2, scenario S6). In production the
  signing key sits in OpenBao transit, where it cannot be exported: notaio may only ask for
  signatures, and every request is audited outside the cluster
  ([ADR-0004](adr/0004-signing-key-custody.md)).
- **The guest decides nothing.** mediatore-guest renders what the bundle says. It never evaluates
  policy and never talks to a Kubernetes API server, so a compromised guest can at worst break its
  own sandbox, which the controls outside the guest (host and gateway) still contain.

## The four kinds

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

**ExposureCeiling** (namespaced, named after its policy). The approved maximum exposure of one
policy: the audiences it may expose (each with the most access and the longest token), its egress
hosts, its tools and its longest lease, plus an approval from the `Security` role
([ADR-0007](adr/0007-exposure-ceiling.md)). A policy without one, or whose exposure it does not
cover, publishes nothing new. Widening a policy therefore takes two approvals: the policy change
and the ceiling change. The controller only reads ceilings.

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
7. The exposure, grants included, must be covered by the policy's ExposureCeiling (L014 to
   L016). If it is not, or there is none, no bundle is compiled, `Ready` is `False` with the
   reason, and `maxExposure` is still reported so the approvers can see what to approve.

The lint rules and their codes are documented at the top of
`crates/sandboxpolicy-core/src/lint.rs`.

## Checking a change before it merges

`notaio lint <paths>` runs the controller's checks on files: `lint_knobs` on every profile and
`evaluate` on every policy with its grants, under the default ceilings and the current time
([ADR-0006](adr/0006-notaio-lint-cli.md)). A policy the controller would reject fails its pull
request instead, with the same lint codes (L001 to L016, including the ceiling checks) and input codes for what
only a set of files can get wrong (I001 to I008: an invalid object, a duplicate, a missing
profile, an orphaned or expired grant, a ceiling for no policy). Profiles come from the files first, then the built-ins. `--format sarif`
feeds GitHub code scanning, so a reviewer sees the finding on the file. It exits 0 when clean, 1 on
an error finding and 2 on unreadable input.

## The controller

`notaio controller`: two reconcilers in one process. The profile reconciler lints and records a digest. The policy
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
- **Checks the bundle twice.** Before signing, a guard independent of the evaluator checks that a
  ceiling exists, the evaluation found no error, every relaxation in the bundle is unexpired, and the
  bundle is not already past `notAfter`. A failure publishes nothing, reports
  `Ready=False, FailOpenRefused`, and counts in `notaio_fail_open_total`, which must stay 0.
- **Observable.** `GET /healthz`, `/readyz` and `/metrics` on port 8080, and Events on transitions
  (a new bundle version, a failure, a grant lapsing or refused, a profile turning invalid). The
  metrics are computed at scrape time from the listener's own caches of profiles, policies and
  grants: policies by `Ready` reason, lint failures, relaxations by phase, age and time to expiry,
  bundle age, and two series that must be 0, `notaio_relaxations_past_expiry` and `notaio_fail_open_total`
  ([ADR-0010](adr/0010-health-metrics-and-events.md)).
- **One leader.** Two replicas run; the one holding the Lease `notaio` in its own namespace
  reconciles, signs and publishes, and the other waits with warm caches. A leader that cannot renew
  within 10 seconds, or finds another holder, exits; a clean shutdown releases the Lease at once. A
  brief overlap from clock skew can at worst republish an older bundle, which consumers' version
  floor refuses ([ADR-0011](adr/0011-leader-election.md)).
- **Scanned and accounted for.** Every published image is scanned with Grype against an SBOM that
  lists every Rust crate. A finding is suppressed only by an OpenVEX statement in `.vex/` backed
  by a fact about the binary, re-checked on every pull request; a fixable High or Critical fails
  the build ([ADR-0012](adr/0012-image-scanning-and-vex.md)).
- **Least privilege.** Read on the four kinds, `patch` on their status, `create/patch` on
  `events.k8s.io` Events, `get/update` on its own Lease, `get/create/patch` on ConfigMaps in named
  namespaces only. It cannot create, edit or delete a policy object, and it holds no hypervisor
  credentials. Its only inbound port is health and metrics.

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

- The controller, the admission policy and the deployment manifests have run on kind (`make kind-e2e`,
  in CI) and on a three-node k0s test cluster, where kube-router also enforced the NetworkPolicy. Not
  yet on a production cluster, and not yet with a real GitOps reconciler as the admitted identity.
- Distribution is decided (ADR-0003: ConfigMap, read by mediatore, relayed to guests), but the
  mediatore side is not built yet (ROADMAP M3).
- A policy that becomes invalid leaves the previous bundle in place until it expires. The bundle
  lifetime is therefore the upper bound on how long a tightening takes to land. Faster tightening is
  open decision 3.
- K5.2 and K5.3 "apply to lab audiences only" for Lab cannot be enforced without a classification of
  audiences. `dataScope` covers the triangle, not this.
- K2.4 (resource limits) is not checked: the model does not define the medium and large sizes.
