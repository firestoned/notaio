# ADR-0020: SandboxAudience, an audience defined by the platform or by a team, signed like a policy

- **Status:** Accepted
- **Date:** 2026-11-20
- **Proposed:** 2026-11-20
- **Deciders:** Erick Bourgeois
- **Amended:** 2026-11-21 (decision 6: an audience that was never published refuses nothing, and
  the platform wins a tie between two changed audiences; decision 7: header values are checked)
- **Related:** ADR-0002 (the bundle contract); ADR-0003 (distribution through a ConfigMap);
  ADR-0004 (signing key custody); ADR-0007 (the ExposureCeiling and Security's approval);
  ADR-0008 (the consumer's version floor); ADR-0013 (audiences in v1alpha2, schema v3);
  ADR-0015 (signed policy delivery); ADR-0017 (audience classification, which stays on the
  ceiling); ADR-0018 (tightening latency). In mediatore: ADR-0055 (the consumer of this kind),
  ADR-0053 phase 1 (`audiences[].namespaces`, which this replaces), ADR-0040 (gateway placement),
  ADR-0054 (which policies a namespace may name).

## Context

An audience has two halves. The **grant** says which sandboxes may use it, with which rules,
endpoints and token lifetime: that is a `SandboxPolicy` entry, bounded by an `ExposureCeiling`
that Security approves, compiled and signed by notaio, and read by mediatore from the signed
bundle alone. The **definition** says what the audience is: how mediatore obtains or mints the
credential, where it places it, which endpoints it is bound to, and the widest rules any grant
may have. The definition lives today in mediatore's own configuration file (`audiences[]` in
`mediatore.yaml`), which has three consequences that do not fit the platform it serves:

- **A change is a broker restart.** Adding a team's audience, rotating its credential file or
  narrowing its ceiling rules is a rollout of the broker. On a platform where every team runs its
  own delivery pipeline, restarts are the thing that does not scale.
- **It is unsigned.** The definition is trusted from whoever can write one ConfigMap. Everything
  else on the authorization path is signed by notaio or by Security (ADR-0015), so the definition
  is the weakest link of a chain that is otherwise verified end to end.
- **It is global.** A team that brings its own credential (a service id it already uses to push
  images to its registry, held in its own OpenBao and synced into its namespace) has nowhere to
  declare it. mediatore's ADR-0053 phase 1 added a namespace map to the global definition, which
  scopes a shared credential but cannot hold one a team owns.

notaio already holds the other half of the audience: the ceiling (`ExposureCeiling.spec.audiences`,
with `dataScope`, ADR-0017) and the grant (`SandboxPolicy.spec.audiences`, ADR-0013). The
definition belongs beside them, delivered and signed the same way.

## Decision

1. **A fifth kind, `SandboxAudience`** (`sandbox.firestoned.io/v1alpha2`, namespaced): the
   definition of one audience.

   A platform audience, in the platform audience namespace, its credential read by the consumer
   from the platform's OpenBao:

   ```yaml
   apiVersion: sandbox.firestoned.io/v1alpha2
   kind: SandboxAudience
   metadata:
     name: anthropic
     namespace: sandbox-platform        # the platform audience namespace (decision 2)
   spec:
     credential:
       backend: secret                  # sts | secret | delegated | entra-obo | none
       placement: gateway               # gateway | proxy
       source:
         openbao: { mount: kv-platform, path: mediatore/anthropic, key: api-key }
       header: x-api-key
     endpoints: ["api.anthropic.com:443"]
     rules:                             # the widest any grant of this audience may be
       - { method: POST, path: "/v1/messages" }
     maxTtlSeconds: 900
   ```

   A team audience, in the team's namespace, its credential a Secret in the same namespace that the
   team syncs from its own OpenBao (with the External Secrets Operator or any other means):

   ```yaml
   apiVersion: sandbox.firestoned.io/v1alpha2
   kind: SandboxAudience
   metadata:
     name: artifactory
     namespace: team-a                  # usable by team-a's sandboxes only (decision 3)
   spec:
     credential:
       backend: secret
       placement: gateway
       format: basic                    # bearer (default) | basic
       source:
         secretRef: { name: af-team-a-svc, usernameKey: username, tokenKey: token }
     endpoints: ["artifactory.example.com:443"]
     rules:
       - { method: GET,  path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
       - { method: HEAD, path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
       - { method: PUT,  path: "/artifactory/api/docker/app-team-a-docker/v2/**" }
     maxTtlSeconds: 3600
   ```

   The fields are the backend vocabulary mediatore implements today (its `AudienceConfig`):
   `backend`, `placement`, `format`, `header`, `extraHeaders`, the `delegated` refresh settings,
   a CA reference for an upstream on a private PKI (`caRef`, a ConfigMap in the same namespace),
   `endpoints` (exact `host:port`, no wildcard) and `rules` (the policy's rule shape). The object
   carries no credential material and no grant: who may use the audience is decided by
   `SandboxPolicy`, and what Security allows by `ExposureCeiling`. The classification stays on the
   ceiling (ADR-0017); a team does not classify its own audience.

2. **The namespace says which kind of audience it is.** One namespace is the **platform audience
   namespace**, named in notaio's controller configuration (`NOTAIO_AUDIENCE_PLATFORM_NAMESPACE`;
   notaio is configured by environment only) and in each consumer's own configuration. `notaio
   lint` takes the same name as `--platform-audience-namespace`. There is no default: the two sides
   must agree, and a consumer derives the kind from the namespace itself, never from a field in the
   bundle. A `SandboxAudience` there is a **platform audience**: usable from any namespace a signed
   grant reaches. A `SandboxAudience` in any other namespace is a **team audience**.

   **Opt-in.** While the platform audience namespace is unset, notaio does not reconcile
   `SandboxAudience`, publishes no audience bundle, and does not apply `L022` to `L029` or the
   definition half of `L014`: policies behave exactly as before this ADR. Setting it turns all of
   them on together. This is what lets an existing installation upgrade, and a consumer run its
   shadow period (mediatore ADR-0055), before any definition exists.

3. **A team audience is locked to its namespace.** It is usable only by a sandbox in the same
   namespace, and granted only by a `SandboxPolicy` in the same namespace. Nothing extends it:
   not a ceiling, not a consumer's mapping of namespaces (mediatore's `policy.sources`), not a
   grant elsewhere. notaio refuses a grant of a team audience from another namespace (`L027`), and
   mediatore applies the same rule to the claim's namespace (its ADR-0055).

4. **What a team audience may be.** `backend` is `secret` or `none`; `sts`, `delegated` and
   `entra-obo` mint, refresh or exchange with the consumer's own signing key, issuer or app
   registration, which are platform material and stay with platform audiences (`L022`). Its
   credential source is a `secretRef`. `secretRef` and `caRef` have no namespace field: each is
   always resolved in the audience's own namespace, so the schema leaves no way to point at another
   one. An `openbao` source is refused in a team namespace, because the consumer reads OpenBao with its own identity and a team must not be able
   to point that identity at a path it chooses (`L023`). `placement: proxy` is allowed and reported
   as a warning (`L029`): the credential then sits in the pool VM's memory, outside the jail.

5. **A platform audience** may use every backend. Its source is `openbao` (the consumer reads the
   platform's secret engine with its own identity) or a `secretRef` in the platform audience
   namespace.

6. **Names and endpoints never resolve two ways.** A team audience may not take the name of a
   platform audience (`L024`): a sandbox naming `anthropic` gets the same definition in every
   namespace, or none. Within a namespace, an endpoint belongs to at most one audience, counting
   the platform audiences (`L025`), so a gateway placing a credential for a claim is never choosing
   between two. notaio sees every namespace and enforces both at reconcile; a consumer refuses the
   same collisions on what it reads, never resolving by order (mediatore's ADR-0054 lesson).
   **The newcomer is refused.** Each audience has a standing, read from its status: *new* (no
   published bundle), *stable* (a published bundle that is its current content: its content digest
   equals `status.bundleDigest`), or *changed* (a published bundle, and content that has moved
   since). Of two colliding audiences:

   | This audience | The other | This audience is |
   | --- | --- | --- |
   | new | anything | refused |
   | stable | new or changed | kept |
   | stable | stable | refused (both are) |
   | changed | new | kept |
   | changed | stable | refused |
   | changed | changed | a platform audience against a team audience is kept, the team audience refused; two of the same kind are both refused |

   So a new audience, platform or team, that takes a working audience's name or endpoint is
   refused and the working one keeps publishing; an edit that moves a published audience into a
   collision is refused the same way, while its last bundle (without the change) serves until its
   lifetime ends (decision 11). At least one side of every collision is refused, so two colliding
   definitions are never both published. `notaio lint` sees no status, so at pull request time
   every audience is new and every collision is refused on both sides.

   **An audience that was never published refuses nothing** (amended 2026-11-21). As first
   written, the rule refused both sides whenever neither was stable. A published audience stops
   being stable the moment it is edited, so a refused newcomer left in place would have turned the
   next edit of the audience it collides with into a finding, and an audience with a finding is no
   longer re-signed. One team namespace could in that way have withdrawn a platform audience from
   every namespace. The standing above closes it: a new audience can only lose, and between two
   changed audiences of different tenants the namespace decides, which no tenant can forge. The
   standing is read from `status`, so `patch` on `sandboxaudiences/status` stays notaio's alone
   (`deploy/controller/10-rbac.yaml`).

7. **Headers are checked.** `header` and each `extraHeaders` name are lowercase RFC 9110 tokens
   and none of `host`, `content-length`, `transfer-encoding`, `connection`, `te`, `upgrade`,
   `trailer`, `keep-alive`, `proxy-authorization` or `proxy-connection` (`L026`). Each
   `extraHeaders` value is printable ASCII, with spaces and tabs inside it only: no carriage
   return, line feed or other control character, which could end the header and start another
   (`L026`, amended 2026-11-21; a name alone was checked before). The values are published in the
   bundle as written, so a credential never belongs in one.

8. **A definition opens nothing on its own; the ceiling still decides.** A policy's exposure for a
   granted audience is now its grant **and** the audience's definition: `L014` compares the
   definition's `endpoints` and `maxTtlSeconds` against the policy's ceiling as well, so a team
   cannot reach an endpoint Security did not approve by writing it into an audience it owns. A
   policy that grants an audience no definition is visible for (a team audience in the policy's
   own namespace, or a platform audience) publishes no bundle (`L027`). The grant's `endpoints`
   must be a subset of the definition's `endpoints`, and an empty list reaches nothing (fail closed,
   as the consumer's effective grant already does); its `rules` must sit inside the definition's
   `rules`, and an empty list means the definition's; and its `ttlSeconds` under the definition's
   `maxTtlSeconds` (`L028`). The grant keeps its own fields, so the policy bundle's schema
   (`bundle/v3`) does not change.

9. **A signed audience bundle, its own type.** Each valid `SandboxAudience` is compiled into a
   DSSE envelope with payload type `application/vnd.firestoned.sandboxaudience.bundle.v1+json`,
   schema `sandbox.firestoned.io/audience-bundle/v1`, canonical form RFC 8785 (ADR-0005), signed
   with the same custody as policy bundles (ADR-0004). Because DSSE signs the payload type, a policy
   bundle can never verify as an audience bundle, nor the reverse. The content is the spec, the
   audience's namespace, name and `uid`, a monotonic version that moves only with the content
   digest, and `issuedAt` and `notAfter` on the policy bundle's lifetime (ADR-0018). Consumers keep
   a version floor per audience namespace and name, with the rules of ADR-0008.

10. **Distribution as for policy bundles** (ADR-0003). notaio publishes the envelope to a
    ConfigMap `<audience>-audience` in the audience's namespace, under `audience.dsse.json`,
    version and digest as annotations, owned by the `SandboxAudience`; notaio is the only writer.
    Deleting the `SandboxAudience` deletes the ConfigMap; a consumer that no longer sees a valid
    bundle treats the audience as gone. The bundle Role notaio already holds per namespace
    (`deploy/policy-namespace/notaio-bundles.yaml`) gains nothing new to write (ConfigMaps); the
    platform audience namespace needs it like any policy namespace. The ClusterRole gains
    read on `sandboxaudiences`, `patch` on `sandboxaudiences/status` and `update` on
    `sandboxaudiences/finalizers` (for the ConfigMap's owner reference), and nothing else:
    notaio never reads the Secrets or ConfigMaps an audience references.

11. **Status like a policy.** `Ready`, `Valid` with the findings, the published version and digest.
    A finding blocks publication of a new version; the last valid bundle stays until its lifetime
    ends, as for a policy (ADR-0018).

12. **Who writes them is a requirement on the deployment, not a check notaio can make.** Platform
    audiences come from the platform's policy repository (the `policies` artifact of ADR-0015). Team
    audiences come from the team's own repository, delivered by GitOps, and each namespace admits
    only its GitOps identity as a writer of `SandboxAudience`, `SandboxPolicy` and the bundle
    ConfigMaps (a `ValidatingAdmissionPolicy`, the pattern mediatore uses for its `AgentSandbox`).
    The ceiling stays Security's alone (ADR-0015).

13. **New stable codes:**

    | Code | Meaning |
    | ---- | ------- |
    | L022 | A team audience with a backend other than `secret` or `none` |
    | L023 | A team audience with an `openbao` source |
    | L024 | A team audience and a platform audience with the same name (on the newcomer, decision 6) |
    | L025 | An endpoint held by two audiences visible from one namespace (on the newcomer, decision 6) |
    | L026 | A header name that is not a lowercase token or is reserved, or a header value that is not printable ASCII |
    | L027 | A policy grants an audience no definition is visible for, or a team audience from another namespace |
    | L028 | A grant's endpoints, rules or token lifetime wider than its audience's definition |
    | L029 | A team audience placed in the proxy (a warning, `Severity::Warning`; it publishes) |

## Security analysis

| Threat | STRIDE | Control |
| --- | --- | --- |
| A team defines an audience whose credential is another team's or the platform's | I/E | `L023`: a team's source is a Secret in its own namespace only, never OpenBao; the consumer applies the same rule on read (mediatore ADR-0055) |
| A team opens egress Security did not approve by writing an endpoint into its own audience | E | `L014` extended (decision 8): the definition's endpoints are compared against the policy's Security-approved ceiling; no grant, no use |
| A team audience used from another namespace | E | Decision 3, `L027` at notaio and the claim's namespace at the consumer |
| A team audience shadows a platform audience, so a name means different things in different namespaces | S/T | `L024`; the consumer refuses a collision instead of choosing |
| Two audiences on one endpoint, so the gateway places the wrong credential | I | `L025`; the consumer refuses the overlap |
| A team uses a minting backend that acts with the platform's keys | E | `L022` |
| A header injected to smuggle or override framing, by its name or through a line break in its value | T | `L026` |
| A team withdraws a platform audience by leaving a refused, colliding audience in its own namespace | D | Decision 6: an audience that was never published refuses nothing, and the platform wins a tie between two changed audiences |
| A policy bundle replayed as an audience bundle, or the reverse | S/T | Distinct DSSE payload types; the signature covers the type |
| A definition changed in the cluster by someone who can write the object | T | Signed delivery of the source (ADR-0015) and the per-namespace admission requirement (decision 12); the consumer trusts the signature, never the object |
| An old definition replayed after a tightening | T | The version floor (ADR-0008) per audience |
| The credential itself changed by whoever can write the Secret | T | Not covered: the bundle signs the reference, not the secret. The same as today's mounted file; the Secret's writers are the team's (or the platform's) own |

## Consequences

- **No restart for any audience change.** A new team audience, a rotated credential, a narrower
  ceiling rule: each is a signed bundle the consumer reads as it arrives.
- **The definition is signed.** The last unsigned part of the authorization path moves under
  notaio's signature and the delivery chain of ADR-0015.
- **Teams own their credentials end to end.** The credential never passes through the platform
  team: the team's OpenBao, the team's sync, the team's namespace, the team's audience.
- **notaio learns the consumer's backend vocabulary.** `backend`, `placement`, `format` and the
  refresh settings are mediatore's concepts. notaio is already sandbox-specific, and the schema is
  versioned; a new backend in mediatore is a field here first.
- **The consumer needs read access to Secrets in team namespaces,** narrowed to the names the
  audiences reference (a `Role` with `resourceNames`, created at onboarding), and read access to
  the platform's OpenBao paths. That is mediatore's side, in its ADR-0055.
- **Migration.** mediatore reads both its file and the bundles during a shadow period and reports
  differences; then its file's `audiences[]` is retired (mediatore ADR-0055).

## Alternatives rejected

- **Keep the definition in mediatore's file.** A restart per change, unsigned, global (Context).
- **A CRD read by mediatore directly.** It would drop the signature: write access to the object
  would be enough to change what a credential is placed for, which is the weakening this ADR exists
  to avoid.
- **One namespace for every audience, team audiences included.** The platform team would hold
  every team's credential and every change would be a ticket to it.
- **A team audience reading OpenBao through the consumer's identity.** The team would choose the
  path that identity reads; one wrong path and a team reads another team's secret through its own
  audience.
- **Classification on the definition.** Security classifies (ADR-0017); a team would classify its
  own audience as lab.

## Resolved questions

Resolved by the deciders on 2026-11-20, on acceptance:

1. **The platform audience namespace's name.** Configurable on both sides, with no default, so a
   mismatch is a refusal rather than two components guessing (decision 2). `sandbox-platform` is
   the name the examples use.
2. **A team audience granted by a policy in a mapped namespace.** Refused (decision 3): a team's
   own audience is granted by its own policy.
3. **Upgrading an installation whose policies grant audiences with no definition.** Opt-in through
   the platform audience namespace (decision 2), not a hard `L027` on upgrade.
4. **The grant's own endpoints.** A subset of the definition's, under `L028`; an empty list reaches
   nothing (decision 8).
5. **A reference outside the audience's namespace.** Ruled out by the schema: `secretRef` and
   `caRef` have no namespace field (decision 4).
6. **Which side of a collision is refused.** The newcomer, by the stable rule of decision 6, so a
   platform audience added later never breaks a team's working audience.

The CALM step is deferred, as for every ADR until the model is bootstrapped: `docs/architecture.md`
carries this change.

## What must be proven

- `sandboxpolicy-core`: the lint rules `L022` to `L029`, each with a passing and a
  failing case, `L014` with a definition's endpoint outside the ceiling, `L028` with a grant
  endpoint outside the definition, and none of them applied while the platform audience namespace
  is unset.
- `sandboxpolicy-bundle`: the audience payload type, canonical bytes pinned by a golden digest, and
  a policy bundle refused as an audience bundle (and the reverse).
- The controller: publishes `<audience>-audience`, deletes it with the object, keeps the last valid
  bundle on a new finding, and reports status.
- `notaio lint` covers the same rules at pull request time.
- The CRD validation schema and the docs: the guide for teams (bringing your own credential) and
  for the platform (the platform audience namespace).
- A threat model pass (`.claude/rules/threat-modeling.md`).
