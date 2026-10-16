# ADR-0005: The bundle content is canonicalised with RFC 8785

- **Status:** Accepted
- **Date:** 2026-10-16
- **Deciders:** Erick Bourgeois
- **Resolves:** [ADR-0002](0002-bundle-contract.md) open decision 4 (canonicalisation)
- **Related:** ADR-0002 decision 2 (content) and decision 5 (verification)

Not implemented. This is the last change to the content's byte form before the bundle schema is
frozen (ROADMAP M1).

## Context

A consumer checks that `contentDigest` matches the content (ADR-0002, decision 5). To do that it
re-serialises the content and hashes the bytes, so every consumer has to produce exactly the bytes
notaio produced.

Today those bytes are `serde_json::to_vec` over Rust structs with a fixed field order and sorted
maps and lists: deterministic, but defined only by our Rust code. A consumer that does not link
`sandboxpolicy-bundle` (a gateway written in another language, an auditor's script) cannot recompute
the digest, and would have to trust it.

RFC 8785, the JSON Canonicalization Scheme (JCS), defines one byte form for any JSON value: object
members sorted by their UTF-16 code units, minimal string escaping, no insignificant whitespace,
and numbers in the ECMAScript form of an IEEE-754 double. Libraries exist in most languages.

## Decision

1. **The canonical bytes of `BundleContent` are its RFC 8785 serialisation.**
   `contentDigest` is `sha256:` followed by the lowercase hex SHA-256 of those bytes.
2. **Content numbers are integers in the I-JSON safe range** (0 to 2^53 - 1), so the double form
   in RFC 8785 is exact. The content already has no floats (ADR-0002). The only field that could
   exceed the range is `budgets.maxEgressBytes` (`u64`). Compilation fails closed for a value above
   2^53 - 1, rather than publishing a digest that another implementation would compute
   differently. The ceilings make such a value unreachable in practice.
3. **The canonical form is implemented in `sandboxpolicy-bundle`,** with the RFC's test vectors as
   tests. The crate keeps its rule of no Kubernetes dependency. A third-party JCS crate is used only
   if it passes `deny.toml` and is maintained; otherwise the small no-float subset is implemented
   in the crate.
4. **This is a schema change.** Per CLAUDE.md, `SCHEMA` becomes `sandbox.firestoned.io/bundle/v2`,
   and v2 is the schema that is frozen. No consumer has accepted a v1 bundle, so nothing has to
   migrate. The DSSE payload type changes with it.
5. **Only the content is canonicalised.** The DSSE signature covers the payload bytes exactly as
   sent (ADR-0002, decision 1), so the envelope and `meta` need no canonical form.

## Consequences

- Any consumer, in any language with an RFC 8785 library, can recompute the digest. The gateway's
  language stops being a constraint on this contract.
- Every content digest changes once, and the field order of `BundleContent` stops being part of the
  canonical form, because JCS sorts members. The comment on the struct that forbids reordering
  is updated accordingly.
- The bound in decision 2 is a new compile-time failure. It needs a test that fails if the bound is
  removed (CLAUDE.md, code rules).
- **CALM step deferred:** notaio has no CALM model yet. The contracts table in
  `docs/architecture.md` records the change.
