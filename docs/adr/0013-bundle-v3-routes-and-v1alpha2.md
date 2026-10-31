# ADR-0013: Bundle v3, routes per audience, and API v1alpha2

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Erick Bourgeois
- **Related:** ADR-0002 (the bundle contract, amended by this), ADR-0005 (canonical form),
  ADR-0007 (exposure ceiling); roadmap 00 (M3), decision D2; mediatore ADR-0025 (the egress proxy)
  and its roadmap 07

## Context

mediatore's egress proxy reaches the outside world through **routes**: each audience a sandbox may
get a token for has endpoints (`host:port`), and the audience's **rules**, pairs of an HTTP method
and a path glob, bound what may be called there. The proxy injects the audience's token, so every
such call is brokered. mediatore renders nftables, the proxy and Landlock from these routes, and its
roadmap asks for them to come from one signed policy: a notaio bundle.

Bundle v2 cannot carry them. Its audiences have a name, an access level, scopes and a token
lifetime, but no endpoints; its `egress[]` is direct egress with a separate `methods[]` and
`paths[]` whose cross product is the rule. Translating between the two in mediatore would make two
renderings of one policy, which is what M3 removes.

## Decision

1. **Each audience carries its endpoints and rules.** `audiences[].endpoints` lists exact
   `host:port` names; `audiences[].rules` lists `{ method, path }`: a method or `*`, and a path that
   starts with `/`, where `*` matches one segment and `**` zero or more, matched without the query
   (mediatore ADR-0025, decision 4). No rules means every method and path, as in mediatore.
2. **Audience endpoints are brokered, so any K4.1 step allows them.** K4.1 R0 means all calls are
   brokered; these are. They count under K5.3's audience set, as audiences do today. No wildcard
   host and no IP literal, ever.
3. **Direct egress takes the same shape.** `egress[]` becomes `{ endpoint, rules }`, one rule
   language for the whole bundle. Its existing limits stay: none at K4.1 R0, rules from R2,
   wildcard hosts at R3, never an IP literal.
4. **One endpoint, one way.** An endpoint in two audiences, or in an audience and in direct egress,
   is refused: mediatore's broker refuses the first, and the second would leave the proxy and
   nftables disagreeing about how it is reached.
5. **Two lint rules.** `L017`: an audience endpoint or rule is malformed. `L018`: an endpoint is
   reachable two ways. `L007` checks direct egress in its new shape.
6. **The exposure and its ceiling follow.** `status.maxExposure.audiences[]` and the
   ExposureCeiling's `audiences[]` gain `endpoints`; an audience endpoint the ceiling does not list
   for that audience is an `L014` finding, so widening where a token may be used needs Security's
   approval, as widening the audiences does. Rules narrow and are not in the ceiling.
7. **Bundle schema v3.** `SCHEMA` becomes `sandbox.firestoned.io/bundle/v3` and the payload type
   `application/vnd.firestoned.sandboxpolicy.bundle.v3+json`. A v2 consumer refuses a v3 bundle
   rather than misreading it. Endpoints and rules are sorted in the canonical form.
8. **API v1alpha2, and v1alpha1 retired.** All four kinds move to `sandbox.firestoned.io/v1alpha2`
   together, so the group has one version. The CRDs serve and store v1alpha2 only: serving both
   would need a conversion webhook, an inbound listener ADR-0010 does not allow. Objects written as
   v1alpha1 are re-applied as v1alpha2 at upgrade.

## Consequences

- mediatore can render routes, nftables and Landlock from the verified bundle alone (roadmap 00,
  phase 3), and its broker configuration stops being the policy.
- An upgrade is not in place: apply the new CRDs, then re-apply every policy object as v1alpha2
  through GitOps. The release notes and the install guide say so. No consumer reads bundles yet, so
  no consumer breaks.
- The policy repository's files change shape: `apiVersion`, audiences with endpoints, and direct
  egress as `{ endpoint, rules }`. `notaio lint` refuses the old shapes with input findings.
- The ceiling becomes more precise: approving an audience no longer approves every host it might
  be reached at.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
