# anatomy.md

> Auto-maintained by OpenWolf. Last scanned: 2026-09-30T20:07:26.362Z
> Files: 69 tracked | Anatomy hits: 0 | Misses: 0

> Project structure index. Auto-maintained by OpenWolf hooks and daemon.
> Run `openwolf scan` to generate, or wait for the first Claude Code session.
> Status: Pending initial scan

## ./

- `.gitignore` — Git ignore rules (~512 tok)
- `AGENTS.md` — OpenWolf (~75 tok)
- `Cargo.toml` — Rust package manifest (~149 tok)
- `CLAUDE.md` — OpenWolf (~1125 tok)
- `clippy.toml` (~17 tok)
- `deny.toml` — cargo-deny configuration. The licence list was derived from `cargo metadata` of the current (~205 tok)
- `Makefile` (~3747 tok)
- `README.md` — Project documentation (~773 tok)
- `ROADMAP.md` — Roadmap (~731 tok)

## .claude/

- `CHANGELOG.md` — Changelog (~508 tok)

## .claude/rules/

- `architecture-driven-development.md` — Architecture Driven Development (ADD) (~1470 tok)
- `documentation.md` — Documentation Standards (~944 tok)
- `rust-style.md` — Rust Style Guide (~1226 tok)
- `testing.md` — Testing Standards (~1179 tok)
- `threat-modeling.md` — Threat Modeling (~1153 tok)

## .github/workflows/

- `ci.yaml` — CI: ci (~175 tok)

## crates/notaio/tests/

- `e2e_admission.rs` — e2e: `deploy/admission/gitops-only.yaml` refuses direct edits to policy objects, lets the GitOps (~1732 tok)
- `e2e_bundle.rs` — e2e: a policy goes in, a signed bundle comes out, and a consumer can verify it. (~2108 tok)
- `e2e_rbac.rs` — e2e: the controller's deployed RBAC is read plus status writes, and nothing more. (~1446 tok)

## crates/notaio/tests/common/

- `mod.rs` — Shared helpers for the kind e2e suites (`make kind-e2e`, `.github/workflows/e2e.yaml`). (~1470 tok)

## crates/sandboxpolicy-api/

- `Cargo.toml` — Rust package manifest (~184 tok)

## crates/sandboxpolicy-api/src/

- `grant.rs` — A recorded approval by a role named in the knob register. `reference` points at the reviewed (~676 tok)
- `lib.rs` — CRD types for the SandboxPolicy API, `sandbox.firestoned.io/v1alpha1`. (~226 tok)
- `policy.rs` — Identity provider group ids allowed to claim a sandbox under this policy. (~1109 tok)
- `profile.rs` — A named bundle of knob steps (Fortress, Hardened, Standard, Lab). Cluster-scoped and (~503 tok)
- `status.rs` — A minimal condition type. Kept local so the CRD schema does not depend on the time (~192 tok)

## crates/sandboxpolicy-bundle/

- `Cargo.toml` — Rust package manifest (~151 tok)

## crates/sandboxpolicy-bundle/src/

- `content.rs` — PolicyRef: canonical_bytes, digest, new (~796 tok)
- `dsse.rs` — DSSE envelope, https://github.com/secure-systems-lab/dsse (~929 tok)
- `error.rs` — [derive(Debug, Error, PartialEq, Eq)] (~235 tok)
- `lib.rs` — The signed policy bundle: the contract between sicario (producer) and mediatore, (~327 tok)
- `verify.rs` — The set of public keys a consumer trusts, by key id. (~895 tok)

## crates/sandboxpolicy-bundle/tests/

- `bundle.rs` (~1688 tok)

## crates/sandboxpolicy-core/

- `Cargo.toml` — Rust package manifest (~186 tok)

## crates/sandboxpolicy-core/src/

- `builtin.rs` — The four built-in profiles, transcribed from the profile matrix in section 10 of the threat (~708 tok)
- `ceilings.rs` — Numeric ceilings that hang off knob steps. (~590 tok)
- `evaluate.rs` — `evaluate` is the whole decision: given a policy, its profile and its grants at an instant, say (~3678 tok)
- `lib.rs` — Pure policy logic: the knob register, the built-in profiles, the lint rules, the ceilings and the (~163 tok)
- `lint.rs` — Lint rules. Every rule has a stable code so findings can be documented, tested and alerted on. (~4466 tok)
- `registry.rs` — The knob register, transcribed from section 9 of the AgentSandbox threat model. (~1097 tok)

## crates/sandboxpolicy-core/tests/

- `evaluate.rs` (~3322 tok)

## crates/sandboxpolicy-types/

- `Cargo.toml` — Rust package manifest (~123 tok)

## crates/sandboxpolicy-types/src/

- `knob.rs` — A relaxation step for a knob. `R0` is the locked default; higher steps are progressively looser. (~556 tok)
- `lib.rs` — Pure data types shared by the SandboxPolicy CRDs, the compiler and the signed bundle. (~125 tok)
- `parts.rs` — Classification of the data a profile may touch. It exists so the toxic-combination rule can be (~683 tok)

## crates/sicario/

- `Cargo.toml` — Rust package manifest (~293 tok)

## crates/sicario/src/

- `error.rs` — [derive(Debug, Error)] (~104 tok)
- `main.rs` — Sicario: validates SandboxProfile, SandboxPolicy and KnobGrant objects, compiles each (~1027 tok)
- `reconcile.rs` — Ctx: bundle_object_name, reconcile_policy, reconcile_profile, error_policy (~2585 tok)
- `signer.rs` — Loads the development signer from `SICARIO_SIGNING_KEY_FILE` (base64 of a 32 byte seed) and (~323 tok)

## crates/sicario/src/bin/

- `crdgen.rs` — Generates the raw manifests that are checked in under `deploy/`. (~449 tok)

## crates/sicario/tests/

- `manifests.rs` — The checked-in YAML must parse into the typed objects and behave under the real evaluator. (~584 tok)

## deploy/admission/

- `gitops-only.yaml` — Direct edits to policy objects are refused: changes arrive only through review and GitOps (~355 tok)

## deploy/controller/

- `00-namespace.yaml` — K8s Namespace: sicario-system (~101 tok)
- `10-rbac.yaml` — K8s ServiceAccount: sicario (~558 tok)
- `20-deployment.yaml` — K8s Deployment: sicario (~546 tok)
- `30-networkpolicy.yaml` — Sicario takes no inbound connections and makes outbound calls only to DNS and the Kubernetes (~204 tok)

## deploy/crds/

- `sandbox.firestoned.io.yaml` — K8s CustomResourceDefinition: sandboxprofiles.sandbox.firestoned.io (~5170 tok)

## deploy/profiles/

- `builtin.yaml` — K8s SandboxProfile: fortress (~786 tok)

## docs/

- `architecture.md` — Architecture (~1331 tok)
- `framework-mapping.md` — Mapping to the FINOS AI Governance Framework (~4117 tok)
- `threat-model-mapping.md` — Mapping to the AgentSandbox threat model (~683 tok)

## docs/adr/

- `0001-separate-project.md` — ADR-0001: Sicario is a separate project (~483 tok)
- `0002-bundle-contract.md` — ADR-0002: The bundle contract (~758 tok)

## docs/air/

- `catalogue.yaml` (~1582 tok)
- `mapping.yaml` — One entry per item in docs/air/catalogue.yaml. Judgments are the author's and need review by (~2822 tok)

## examples/

- `knobgrant-package-install.yaml` — Relax K2.3 (package installation) from R0 to R1 for one policy, for 30 days. R1 grants last at most (~198 tok)
- `sandboxpolicy-hardened.yaml` — K8s SandboxPolicy: agent-default (~139 tok)

## scripts/

- `check_air_mapping.py` — Validate docs/air/mapping.yaml against the repo and render docs/framework-mapping.md. (~1916 tok)
