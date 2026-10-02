# notaio

The policy compiler and validator for the AgentSandbox platform. It turns a small, reviewable
`SandboxPolicy` into a signed, versioned bundle that mediatore, mediatore-guest and the egress
gateway enforce, and it refuses any policy that breaks the threat model's rules.

> Status: **v0 scaffold.** The pure logic (registry, lint, compiler, bundle signing and verification)
> is implemented and tested. The controller compiles and its decision logic is tested, but it has
> **not been run against a cluster**, and neither have the admission policy or the deployment
> manifests. Treat those as reviewed drafts. See [ROADMAP.md](ROADMAP.md).

## What it does

```
SandboxProfile (cluster, platform owned)   knob steps for Fortress, Hardened, Standard, Lab
SandboxPolicy  (namespaced)                binds groups to a profile, adds audiences, egress, tools
KnobGrant      (namespaced)                a time-boxed, approved, one-step relaxation of one knob
        |
        v   notaio controller: lint, apply grants, compute ceilings, compile, sign
Signed bundle (DSSE envelope in a ConfigMap) + status: version, digest, maxExposure, conditions
        |
        v   consumed, never edited
mediatore (verify, evaluate at every mint)   mediatore-guest (render jail, nftables, settings)   gateway
```

It is deliberately **not** in banlieue (a provider-agnostic VM API) and **not** in mediatore (the most
trusted runtime component). See [ADR-0001](docs/adr/0001-separate-project.md).

## Layout

| Path | Purpose |
| --- | --- |
| `crates/sandboxpolicy-types` | Pure data types. No Kubernetes dependency. |
| `crates/sandboxpolicy-api` | The three CRDs, `sandbox.firestoned.io/v1alpha1`. Banlieue and tooling depend on this. |
| `crates/sandboxpolicy-bundle` | Bundle format, DSSE signing, verification. **The only crate mediatore-guest needs.** |
| `crates/sandboxpolicy-core` | Knob register, built-in profiles, lint rules, ceilings, `evaluate`. Pure, no I/O. |
| `crates/notaio` | The controller, and `crdgen` which generates the checked-in manifests. |
| `deploy/` | Raw manifests. `crds/` and `profiles/` are generated, the rest is hand written. |
| `docs/` | Architecture, ADRs, mapping to the threat model, and the [FINOS AI Governance Framework mapping](docs/framework-mapping.md) (generated from `docs/air/`). |

## Develop

```sh
make test        # 39 tests
make lint        # rustfmt and clippy with warnings denied
make manifests   # regenerate deploy/crds and deploy/profiles from the code
```

Rust 1.89 or newer. Every dependency is permissive (see `deny.toml`).

## Try the compiler without a cluster

The decision logic is a pure function, so you can exercise it from a test. The fastest way to see it
work is to read `crates/sandboxpolicy-core/tests/evaluate.rs`: each test is a scenario from the
threat model (a grant that jumps two steps, a grant that completes the exfiltration triangle, a
missing approval, an expired grant).

## Licence

Apache-2.0 is set in `Cargo.toml` as an assumption. Confirm it, and add a `LICENSE` file, before the
first public push.
