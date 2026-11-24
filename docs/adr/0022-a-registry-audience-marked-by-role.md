# ADR-0022: A registry audience, marked by `role` on the definition

- **Status:** Proposed
- **Date:** 2026-11-23
- **Deciders:** Erick Bourgeois
- **Related:** Extends [ADR-0020](0020-sandbox-audience.md) (the `SandboxAudience` definition and
  its signed bundle); ADR-0005 (canonical form); ADR-0008 (the consumer's version floor); ADR-0007
  (the ceiling still decides). In mediatore: ADR-0063 (the image pull goes through the claim's proxy
  and the gateway, whose decision 2 and open question 1 call for this marker), ADR-0055 (the
  consumer of audience bundles), threat-model row R-63. Threat model T4.1, T5.3.

## Context

mediatore's ADR-0063 makes an image registry an audience like any other: the pool's puller
reaches it through the claim's egress proxy and a gateway, and a claim may pull only from a
registry its namespace was granted. It answered its open question 1 with a marker on the audience,
`role: registry`, so that:

- a team cannot pull images from a host it was granted only for API calls, nor call as an API a
  host it was granted only to pull from (the broker serves a registry audience's routes to the
  puller alone, `404 pull_only` to the workload);
- a registry audience's rules can be held to reading the registry API, `GET` and `HEAD` under
  `/v2/`.

mediatore built the marker in its own file configuration (`audiences[].role: api | registry`).
`AudienceDefinition` in `sandboxpolicy-types` has no such field, so a broker that takes its
audiences from signed bundles (mediatore ADR-0055, `mode: bundles`) has no registry and, with
gateways, refuses every image. That is fail closed, and it is also the reason no installation with
gateways can move off the file: the registry is the one audience a bundle cannot yet express.

Two facts about the current code shape the decision:

- `AudienceBundleContent` is in RFC 8785 canonical form, and its doc comment says adding a field is
  a schema change. Every bundle published today carries
  `sandbox.firestoned.io/audience-bundle/v1`.
- No type in `sandboxpolicy-types` or `sandboxpolicy-bundle` denies unknown fields. A consumer built
  against v0.8.0 that reads a definition carrying a new field drops it. Today such a bundle is
  refused anyway, because `verify_audience_bundle` recomputes the content digest from what it
  deserialised and the dropped field changes it (`DigestMismatch`). That refusal is a side effect,
  not a decision, and its error names the wrong cause. mediatore's R-63 records the risk the other
  way round: an old broker serving a registry audience to the workload as an ordinary route.

## Decision

1. **`role` on `AudienceDefinition`: `api` or `registry`, default `api`.** `api` is every
   audience today: something the workload calls. `registry` is an image registry the sandbox's
   image is pulled from; what that means at run time (pull-only routes, the refusal of an image on
   a registry the claim was not granted) is the consumer's, as mediatore ADR-0063 decisions 2 and 5
   have it. notaio signs the marker and lints the definition; it does not enforce the pull.
   `role` is serialised only when it is `registry`, so an `api` definition's canonical bytes, its
   digest and its version are what they are today, and no published bundle changes.

2. **A new stable code, `L030`: a registry audience may only read.** A definition with
   `role: registry` must have at least one rule, and every rule must be the method `GET` or `HEAD`
   on a path that begins `/v2/`, after `L017`'s checks on the rule. An empty `rules` list, which
   for an `api` audience means every method and path, is a finding here: a registry's ceiling is
   written out. It applies to platform and team audiences alike, in the controller and in
   `notaio lint`, and like `L022` to `L029` only while the platform audience namespace is set.
   mediatore's own check (ADR-0063 decision 2) is the same rule; it allows an empty list, which
   this lint does not, so a bundle is never wider than a file audience could be.

3. **A registry definition is published under schema `audience-bundle/v2`; an API one stays `v1`.**
   The payload type, the signing and the canonical form are unchanged. The schema string says which
   shape the content has:

   | Definition | Schema |
   | --- | --- |
   | `role: api` (the field absent) | `sandbox.firestoned.io/audience-bundle/v1`, byte for byte as today |
   | `role: registry` | `sandbox.firestoned.io/audience-bundle/v2` |

   `verify_audience_bundle` accepts both, and refuses a `v1` payload whose definition carries
   `role` and a `v2` payload whose definition is not `role: registry`, with `BundleError::Schema`.
   A consumer built before this ADR refuses every `v2` bundle at the schema check, before it looks
   at the content: the registry audience is absent there (no route, no grant, every image on it
   refused), never served as an API. The digest check stays the second line, not the first.

4. **The grant needs nothing new.** A `SandboxPolicy` grants a registry audience as it grants any
   other. `L028` keeps the grant's rules inside the definition's, so a grant of a registry
   audience reads only; `L014` keeps the definition's endpoints inside the policy's ceiling, so
   Security still approves every registry host. The policy bundle's schema does not change.

5. **Teams may define registries.** A team audience may carry `role: registry` (mediatore
   ADR-0063 decision 2, a registry locked to the team's namespace). ADR-0020 decision 4's limits
   on a team audience's backend and source apply unchanged.

6. **`Role` is a printed column** of `SandboxAudience`, beside `Backend` and `Placement`.

7. **Rollout.** Because an older consumer refuses `v2` cleanly, notaio may be released before or
   after the consumer moves: a registry audience published before the broker reads it is absent
   there, fail closed, and appears once the broker's pin moves. mediatore R-63's ordering (the
   broker first) is no longer a requirement, only the order in which registries start working.

The CALM step is deferred: notaio has no CALM model yet, so `docs/architecture.md` takes the change
(ADD rule, step 2).

## Security analysis

| Threat | STRIDE | Control |
| --- | --- | --- |
| A team pulls images from a host it was granted for API calls, or calls as an API a host it was granted to pull from | E | The marker is signed in the definition (decision 1); the consumer serves the two roles apart (mediatore ADR-0063 decision 5) |
| A registry audience opened for writing (push, delete) | T/E | `L030` (decision 2) on the definition and `L028` on every grant of it; the consumer's own check repeats it on read |
| A consumer that predates the marker serves a registry audience to the workload as an ordinary route | E | Decision 3: a registry definition is `v2`, which an older consumer refuses at the schema check |
| A registry definition replayed as an API one, or the reverse | T | `role` is in the signed content; decision 3's pairing of schema and role refuses a mismatch |
| A team adds a registry host Security never approved | E | `L014` against the policy's ceiling, unchanged (decision 4) |
| Every published API bundle changes and is refused by consumers mid-rollout | D | `role: api` is not serialised and stays `v1` (decision 1); no existing digest or version moves |

## Consequences

- An installation with gateways can define its registries in `SandboxAudience` and move mediatore
  to `policy.audience_definitions.mode: bundles`. That is the change this ADR exists for.
- The audience bundle has two schemas, chosen by content. Consumers must accept both; a consumer
  that knows only `v1` keeps working for every API audience.
- The conformance suite gains `audience-bundle-v2.schema.json` and vectors for a registry bundle.
  `v1`'s JSON schema does not forbid unknown fields, so it would accept a `role`; refusing `role`
  in a `v1` payload is the verifier's check (decision 3), and the vectors pin it.
- mediatore's follow-up, under its ADR-0063 and ADR-0055 rather than a new ADR: the pin moves,
  `role` is read from the bundle, ADR-0063's registry check runs on bundle definitions, the
  shadow comparison includes `role`, and R-63 closes.
- A third role later (for example a package mirror the workload reads but may not write) is
  another value and another schema version, decided by its own ADR.

## Alternatives rejected

- **Every audience bundle moves to `v2` at once.** Forces every consumer to upgrade before notaio,
  and republishes every audience for no change in what it says.
- **Keep `v1` and rely on the digest check to refuse a dropped field.** It refuses today, by
  accident, with an error that blames the digest. A future consumer that deserialises into a
  looser type would lose it silently.
- **Mark the registry on the grant instead of the definition.** The role is what the host is, not
  what one policy wants from it; on the grant, two policies could disagree about the same
  endpoint, and the gateway places credentials by endpoint (ADR-0020 decision 6).
- **Infer a registry from its rules (`/v2/` paths).** An API can serve `/v2/`; the marker must be
  stated and signed, not guessed.

## What must be proven

- `sandboxpolicy-types`: `role` defaults to `api`, is omitted when `api`, and round-trips
  `registry`; a definition without the field deserialises as before.
- `sandboxpolicy-core`: `L030` with a passing case and each failing case (no rules, a `PUT`, a `GET`
  outside `/v2/`), on a platform and on a team audience, and not applied while the platform audience
  namespace is unset; `L028` refusing a grant of a registry audience wider than its definition.
- `sandboxpolicy-bundle`: a golden digest pinning that an `api` definition's canonical bytes did not
  change; a registry definition signed as `v2`; `v1` with `role` and `v2` without it refused with
  `BundleError::Schema`; a v0.8.0-shaped verifier refusing a `v2` bundle (the conformance vectors).
- The controller publishes a registry audience as `v2`, and the `Role` column shows it.
- `notaio lint` reports `L030` at pull request time.
- Docs: the audience guide (a registry audience), the reference, `docs/architecture.md`, the lint
  table in `docs/threat-model-mapping.md`, and a threat model pass
  (`.claude/rules/threat-modeling.md`).
