# Roadmap 00: M3, integrate the consumers

- **Status:** Planned. Nothing here is built; the decisions in section 3 come first.
- **Milestone:** M3 in [`ROADMAPS.md`](../../ROADMAPS.md)
- **Related:** ADR-0002 (the bundle contract), ADR-0003 (distribution), ADR-0004 (signing keys),
  ADR-0008 (the consumer's version floor); mediatore ADR-0008 (AgentSandbox), ADR-0024 (Landlock),
  ADR-0025 (the egress proxy) and its roadmap 07; banlieue ADR-0109 (sandbox network and egress)

## 1. Goal

Today the broker's configuration is the policy. notaio publishes a signed bundle beside it that
nobody reads (mediatore `docs/guides/research-agent-walkthrough.md`, section 5). M3 moves the
authority into the bundle: mediatore verifies it at every mint and enforces it, the guest verifies
it again and renders the jail from it, and the gateway's allowlist comes from it. Nothing a
consumer does may be looser than the bundle it verified.

## 2. Where things stand

Facts as of 2026-10-30, from the mediatore and banlieue trees.

| Area | Today | File |
| --- | --- | --- |
| Policy source | the broker's `audiences[]` config: per audience a backend, `max_ttl`, `egress` endpoints and `rules` | mediatore `crates/mediatore/src/config.rs` |
| Claim | `AgentSandboxSpec { subject, pool_ref, ttl_seconds, audiences, workload }`; no policy field | mediatore `crates/mediatore-claims/src/sandbox.rs` |
| Claim record | no policy or version field | mediatore `crates/mediatore-store/src/lib.rs` |
| Egress | `Route { audience, endpoint, rules }`, `EndpointRule { method, path }` with `*`/`**` segment globs; every endpoint belongs to one audience; no host wildcards | mediatore `crates/mediatore-proto/src/lib.rs` |
| Guest input | `GET /v1/me` over SPIFFE mTLS only; renders nftables, the egress proxy routes, nsjail and Landlock from it | mediatore `crates/mediatore-guest/src/guest.rs` |
| Signatures | none verified anywhere; image layers by digest only | |
| Tokens | ES256; `iss sub aud exp iat nbf jti act claim_uid upstream_iss groups`; no policy version, scopes or access | mediatore `crates/mediatore-sts/src/lib.rs` |
| TTL | per-audience `max_ttl` is collapsed to the maximum across audiences when the STS is built | mediatore `crates/mediatore/src/main.rs` |
| Group eligibility | one `required_group` per issuer; no admission by group anywhere | mediatore `crates/mediatore-entra/src/lib.rs` |
| Kubernetes | kube 4, k8s-openapi 0.28; watches AgentSandbox; reads no ConfigMaps; no RBAC for them | mediatore `deploy/base/rbac.yaml` |
| Dependencies | edition 2024, MSRV 1.96; `unknown-git = "deny"`, git allowed only for banlieue; `ring` and `openssl-sys` banned; no `ed25519-dalek` yet; sha2 0.11, base64 0.23, serde_json 1 match notaio's | mediatore `deny.toml`, `Cargo.toml` |
| banlieue claims | `VirtualMachineClaimSpec { pool_ref, subject, ttl_seconds }`; no policy; the AgentSandbox CRD moved to mediatore | banlieue `crates/banlieue-api/src/banlieue/virtualmachineclaim.rs`, ADR-0055 |

Two things in the existing plans are out of date and are corrected here:

- **"Banlieue: claims name a policy and profile"** (ROADMAPS M3). The claim a person makes is
  mediatore's `AgentSandbox` now; banlieue's `VirtualMachineClaim` is made by the broker. The policy
  is named on the `AgentSandbox`.
- **"Whether the gateway also verifies the notaio bundle itself is open in mediatore ADR-0023"**
  (banlieue ADR-0109). mediatore's ADR-0023 is the Kairos VM image; the gateway ADR it means
  (`0023-egress-gateway-envoy-with-spiffe-identity`) is not in the mediatore tree, and its number is
  taken. The question has no home yet.

## 3. Decisions needed first

Each is an ADR in the repository that owns it. Nothing in section 4 starts before D1 to D3.

**D1. How consumers get the crates** (notaio). The workspace is `publish = false`.

- *Recommended:* a git dependency pinned to a release tag,
  `sandboxpolicy-bundle = { git = "https://github.com/firestoned/notaio", tag = "v0.3.0" }`, with
  notaio added to mediatore's `allow-git`. Releases are already signed and attested; a tag is the
  unit consumers pin.
- *Alternative:* publish `sandboxpolicy-types` and `sandboxpolicy-bundle` to crates.io. Better for
  third parties, but a public API commitment before 1.0.
- Either way, `sandboxpolicy-types` puts `schemars` behind a feature, so the guest, which runs as
  root in every VM, does not compile the CRD schema machinery.

**D2. The egress model in the bundle** (notaio, schema v3, amends ADR-0002). *Decided:*
[ADR-0013](../../docs/adr/0013-bundle-v3-routes-and-v1alpha2.md), as recommended below, with the
API moving to v1alpha2. The bundle's
`audiences[]` carry no endpoints, and its `egress[]` is direct, with `methods[] x paths[]`.
mediatore's proxy has no direct egress: every endpoint belongs to an audience, and rules are
`(method, path-glob)` pairs. Options:

- *Recommended:* bundle v3 gives each audience its `endpoints[]` and `rules[]` in mediatore's
  shape (exact `host:port`, `{ method, path }` with `*` one segment and `**` any), and keeps a
  separate, normally empty, `egress[]` for direct hosts a knob step allows. Lint checks rules the
  same way the broker does (method `*` or letters, path starting `/`), and an endpoint in two
  audiences is refused, as the broker refuses it.
- *Alternative:* keep v2 and let mediatore translate. Rejected in advance: two renderings of one
  policy is what M3 removes.

The ExposureCeiling gains endpoints per audience to match (an L014 extension).

**D3. Where the policy is named, and who may use it** (mediatore, amends its ADR-0008).

- `AgentSandbox.spec.policyRef`, immutable like the rest of the spec, resolved in the claim's
  namespace; the claim record keeps the policy's uid and the bundle version it was admitted at.
- Eligibility (K7.2): a ValidatingAdmissionPolicy on `AgentSandbox` with the named `SandboxPolicy`
  as its parameter, admitting the request only if the requester is in `spec.subjects.groups`. It
  replaces nothing yet; the issuer's `required_group` stays as an outer gate.

**D4. How the guest gets the bundle and the keys** (mediatore).

- The bundle rides `GET /v1/me` unchanged, as ADR-0003 already says: the broker relays the exact
  envelope it verified.
- The trusted public keys must not come from the broker: a broker that could choose the keys
  could choose the policy. They come in the VM image (the Kairos derivative of mediatore ADR-0023),
  or through banlieue's `guestinfo` at boot. Recommended: in the image, so a key rotation is an
  image release, which is already signed.

**D5. Whether the gateway verifies the bundle** (mediatore; the ADR banlieue ADR-0109 points to,
which needs writing). Recommended: no. The gateway authorizes on the broker's per-claim answer,
and the broker is the verifier. A gateway that verified too would need the keys, the floor and
the policy namespace, for no gain over mTLS to a broker that already did it.

**D6. What a token carries** (mediatore). A `policy` claim with namespace, name, uid and bundle
version, so a resource server or an audit can tell which policy a token was minted under; and
whether `access` and `scopes` from the bundle become token claims (`scp`), which the walkthrough
lists as not expressed today.

## 4. Work, in order

Each phase is shippable and leaves the system no looser than before.

### Phase 0: notaio, the contract consumers build on

- [ ] D1 decided; `schemars` behind a feature in `sandboxpolicy-types`.
- [x] D2 decided ([ADR-0013](../../docs/adr/0013-bundle-v3-routes-and-v1alpha2.md)): bundle
      schema v3 with endpoints and rules per audience and direct egress as `{ endpoint, rules }`,
      lint `L017` and `L018`, the ExposureCeiling extended, the API at v1alpha2.
- [ ] Conformance vectors: signed sample bundles in the repository with the outcome each must give
      (accepted, forged, expired, downgraded, a stale incarnation, a digest mismatch), generated
      and checked in CI, so mediatore and the guest test against the same files notaio does.
- [ ] A JSON Schema for the v3 payload, generated from the types.
- [ ] A release, v0.3.0, which is what consumers pin.

### Phase 1: mediatore reads and verifies, in shadow

- [ ] A Role per policy namespace for `get`, `list` and `watch` on ConfigMaps labelled
      `app.kubernetes.io/managed-by=notaio`.
- [ ] Watch the bundle ConfigMaps, verify each with `verify_bundle`, keep a floor (a `VersionFloor`
      over `mediatore-store`, so it survives restarts with the claims).
- [ ] Compare each claim's broker configuration with its bundle and log and count every place the
      broker would allow more. Enforce nothing yet: this phase finds the gaps safely.

### Phase 2: mediatore enforces the bundle

- [ ] D3: `AgentSandbox.spec.policyRef`, and the eligibility admission policy.
- [ ] At claim: the policy's audiences are the ceiling for the claim's; the lease is the ceiling
      for `ttl_seconds`; a claim under a policy with no valid bundle is refused.
- [ ] At every mint: the audience is in the verified bundle, the TTL is the bundle's (which also
      fixes the collapsed per-audience `max_ttl`), and the token carries the policy version (D6).
      A bundle that expired or was superseded since the claim refuses the mint.
- [ ] The broker config's `egress` and `rules` are removed, or kept only as a ceiling the bundle
      must fit inside.

### Phase 3: the guest verifies and renders

- [ ] D4: `GET /v1/me` carries the envelope; the trusted keys ship in the VM image.
- [ ] The guest verifies the envelope and keeps its own floor before rendering anything; a bundle
      it refuses means no workload starts.
- [ ] nftables, the proxy routes and Landlock are rendered from the bundle's v3 egress model, one
      rendering for both layers, as mediatore roadmap 07 asks.

### Phase 4: the gateway

- [ ] D5 recorded in mediatore. The gateway's per-claim allowlist, answered by the broker, comes
      from the verified bundle.

## 5. Done when

The research-agent walkthrough's section 5 table can be rewritten with every row's "Under M3"
column moved to "Enforced today", and its sentence "mediatore does not read, verify or relay the
bundle yet" removed. On a cluster: change a policy through GitOps, and the next mint, the next guest
render and the gateway follow it, with a downgraded or forged bundle refused at each.

## 6. Risks

- **A schema change while consumers adopt.** Phase 0 settles v3 before any consumer pins, which is
  why D2 is first.
- **Shadow mode finds the broker allows more than the policies say.** That is its purpose: tighten
  the policies or the broker before phase 2 enforces, not after.
- **Key distribution in the image** couples a key rotation to an image release. The key-version pin
  (ADR-0004, amended) lets a rotation be staged around that release.
