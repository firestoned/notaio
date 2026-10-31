# ADR-0014: How consumers get the bundle crates

- **Status:** Accepted
- **Date:** 2026-10-31
- **Deciders:** Erick Bourgeois
- **Related:** roadmap 00 (M3), decision D1; ADR-0002 (the bundle contract); ADR-0009 (releases)

## Context

mediatore, mediatore-guest and the gateway verify bundles with `sandboxpolicy-bundle`, which
depends on `sandboxpolicy-types`. The workspace is `publish = false`, so neither is on crates.io.
mediatore already takes banlieue as a git dependency pinned by revision, and its `deny.toml` denies
any other git source.

`sandboxpolicy-types` derived `schemars::JsonSchema` on every type for the CRDs, so every consumer
of the bundle crate compiled `schemars` too, including mediatore-guest, which runs as root in every
sandbox VM and has no use for a JSON Schema generator.

## Decision

1. **Consumers depend on a release tag of this repository:**

    ```toml
    sandboxpolicy-bundle = { git = "https://github.com/firestoned/notaio", tag = "v0.3.0" }
    ```

    with `https://github.com/firestoned/notaio` in the consumer's `cargo-deny` `allow-git`. A
    release tag is the unit consumers pin: it has release notes, a signed image and manifest built
    from the same commit, and a `Cargo.lock` entry that records the exact commit.
2. **Nothing is published to crates.io before 1.0.** Publishing commits to a public API and to
    crate ownership; a tag gives consumers the same pin without either.
3. **`schemars` is optional in `sandboxpolicy-types`,** behind a `schema` feature that only
    `sandboxpolicy-api` (the CRDs) turns on. The bundle crate uses the types without it.
4. **`make bundle-deps-check` keeps the bundle crate's own tree small,** failing CI if it ever
    pulls in Kubernetes, `schemars`, an async runtime, networking, TLS, OpenSSL or `ring` (which
    mediatore bans). This also enforces the existing rule that the types and bundle crates never
    depend on `kube`.

## Consequences

- A consumer compiles 36 crates for the bundle crate, none of them networking or async; before,
  `schemars` and its derive came too.
- A change to the bundle crate reaches a consumer only when it moves its tag, on its own schedule,
  with the release notes in hand.
- Consumers build notaio's crates under their own toolchain: the bundle crate keeps a
  conservative MSRV (1.89) and edition 2021, which a newer consumer (mediatore: 1.96, edition
  2024) builds without change.
- **CALM step deferred:** notaio has no CALM model yet.
