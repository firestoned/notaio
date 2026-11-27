# anatomy.md

> Auto-maintained by OpenWolf. Last scanned: 2026-11-27T12:54:10.028Z
> Files: 105 tracked | Anatomy hits: 0 | Misses: 0

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
- `Makefile` — SPDX-License-Identifier: Apache-2.0 (~11628 tok)
- `README.md` — Project documentation (~773 tok)
- `ROADMAP.md` — Roadmap (~731 tok)

## .claude/

- `CHANGELOG.md` — Changelog (~32405 tok)

## .claude/rules/

- `architecture-driven-development.md` — Architecture Driven Development (ADD) (~1470 tok)
- `documentation.md` — Documentation Standards (~944 tok)
- `rust-style.md` — Rust Style Guide (~1226 tok)
- `testing.md` — Testing Standards (~1179 tok)
- `threat-modeling.md` — Threat Modeling (~1153 tok)

## .git/hooks/

- `pre-commit` — Local hook (not tracked): refuse a commit that stages OpenWolf state under .wolf/. (~155 tok)

## .github/

- `dependabot.yml` — SPDX-License-Identifier: Apache-2.0 (~950 tok)

## .github/community/

- `04-composed-policies.md` — Roadmap 04: a policy composed of others, signed as one bundle (~2897 tok)
- `05-bundle-schema-v4.md` — Roadmap 05: bundle schema v4, one migration, consumers first (~3537 tok)

## .github/workflows/

- `ci.yaml` — CI: ci (~175 tok)

## crates/notaio/src/

- `audience.rs` — The `SandboxAudience` reconcile (ADR-0020): lint the definition against every other audience, (~2857 tok)
- `refresh_tests.rs` — SPDX-License-Identifier: Apache-2.0 (~866 tok)
- `refresh.rs` — When a published bundle is re-signed. A bundle is refreshed once half its lifetime is left (~506 tok)

## crates/notaio/tests/

- `e2e_admission.rs` — e2e: `deploy/admission/gitops-only.yaml` refuses direct edits to policy objects, lets the GitOps (~3270 tok)
- `e2e_audience.rs` — e2e: a SandboxAudience goes in, a signed audience bundle comes out (ADR-0020). (~2210 tok)
- `e2e_bundle.rs` — e2e: a policy goes in, a signed bundle comes out, and a consumer can verify it. (~2108 tok)
- `e2e_compose.rs` — e2e: a policy composed of others (ADR-0022), through the deployed controller. (~2707 tok)
- `e2e_rbac.rs` — e2e: the controller's deployed RBAC is read plus status writes, and nothing more. (~1446 tok)
- `e2e_schema_v4.rs` — e2e: bundle schema v4 from the deployed controller (ADR-0026, roadmap 05 phase 4). (~2018 tok)

## crates/notaio/tests/common/

- `mod.rs` — Shared helpers for the kind e2e suites (`make kind-e2e`, `.github/workflows/e2e.yaml`). (~1470 tok)

## crates/sandboxpolicy-api/

- `Cargo.toml` — Rust package manifest (~184 tok)

## crates/sandboxpolicy-api/src/

- `audience.rs` — The definition of one audience (ADR-0020). In the platform audience namespace it is a platform (~667 tok)
- `grant.rs` — A recorded approval by a role named in the knob register. `reference` points at the reviewed (~676 tok)
- `lib.rs` — CRD types for the SandboxPolicy API, `sandbox.firestoned.io/v1alpha1`. (~226 tok)
- `policy.rs` — Identity provider group ids allowed to claim a sandbox under this policy. (~1109 tok)
- `profile.rs` — A named bundle of knob steps (Fortress, Hardened, Standard, Lab). Cluster-scoped and (~503 tok)
- `status.rs` — A minimal condition type. Kept local so the CRD schema does not depend on the time (~192 tok)

## crates/sandboxpolicy-bundle/

- `Cargo.toml` — Rust package manifest (~151 tok)

## crates/sandboxpolicy-bundle/src/

- `audience.rs` — The signed audience bundle (ADR-0020, decision 9): one `SandboxAudience`'s definition, wrapped (~1621 tok)
- `content.rs` — PolicyRef: canonical_bytes, digest, new (~796 tok)
- `dsse.rs` — DSSE envelope, https://github.com/secure-systems-lab/dsse (~929 tok)
- `error.rs` — [derive(Debug, Error, PartialEq, Eq)] (~235 tok)
- `lib.rs` — The signed policy bundle: the contract between sicario (producer) and mediatore, (~327 tok)
- `members.rs` — The members bundle schema v4 adds (ADR-0026 decision 1), and the bundle's own entry types, (~1024 tok)
- `verify.rs` — The set of public keys a consumer trusts, by key id. (~895 tok)

## crates/sandboxpolicy-bundle/tests/

- `audience.rs` — ".into(), (~2584 tok)
- `bundle.rs` (~1688 tok)
- `v4.rs` — Bundle schema v4 beside v3 (ADR-0026): the v4 members, the pairing of payload type and schema, (~3293 tok)

## crates/sandboxpolicy-core/

- `Cargo.toml` — Rust package manifest (~186 tok)

## crates/sandboxpolicy-core/src/

- `audience_tests.rs` — SPDX-License-Identifier: Apache-2.0 (~4042 tok)
- `audience.rs` — Audience definitions (ADR-0020): the lint rules on a `SandboxAudience` (L017 on its endpoints (~3983 tok)
- `builtin.rs` — The four built-in profiles, transcribed from the profile matrix in section 10 of the threat (~708 tok)
- `ceilings.rs` — Numeric ceilings that hang off knob steps. (~590 tok)
- `compose_tests.rs` — A composite on `agent` for `devs`, with the given includes and named parts. (~2787 tok)
- `compose.rs` — A policy composed of others (ADR-0022): `compose` turns a policy and the policies it includes (~3196 tok)
- `evaluate.rs` — `evaluate` is the whole decision: given a policy, its profile and its grants at an instant, say (~3678 tok)
- `lib.rs` — Pure policy logic: the knob register, the built-in profiles, the lint rules, the ceilings and the (~163 tok)
- `lint.rs` — Lint rules. Every rule has a stable code so findings can be documented, tested and alerted on. (~8700 tok)
- `registry.rs` — The knob register, transcribed from section 9 of the AgentSandbox threat model. (~1097 tok)

## crates/sandboxpolicy-core/tests/

- `evaluate.rs` (~3322 tok)

## crates/sandboxpolicy-types/

- `Cargo.toml` — Rust package manifest (~123 tok)

## crates/sandboxpolicy-types/src/

- `audience.rs` — The definition of one audience (ADR-0020): what it is, how the consumer obtains and places its (~1855 tok)
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

- `gitops-only.yaml` — Direct edits to policy objects are refused: changes arrive only through review and GitOps (~939 tok)

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

- `architecture.md` — Architecture (~6317 tok)
- `framework-mapping.md` — Mapping to the FINOS AI Governance Framework (~4117 tok)
- `threat-model-mapping.md` — Mapping to the AgentSandbox threat model (~4860 tok)

## docs/adr/

- `0001-separate-project.md` — ADR-0001: Sicario is a separate project (~483 tok)
- `0002-bundle-contract.md` — ADR-0002: The bundle contract (~758 tok)
- `0017-audience-classification.md` — ADR-0017: Audience classification, so Lab policies reach lab audiences only (~1598 tok)
- `0018-tightening-latency.md` — ADR-0018: Tightening latency: a shorter, bounded bundle lifetime (~1496 tok)
- `0019-resource-sizes.md` — ADR-0019: Resource sizes for K2.4 (~1488 tok)
- `0020-sandbox-audience.md` — ADR-0020: SandboxAudience, an audience defined by the platform or by a team, signed like a policy (~4717 tok)
- `0021-status-written-only-by-notaio.md` — ADR-0021: Status is written only by notaio (~1031 tok)
- `0022-a-registry-audience-marked-by-role.md` — ADR-0022: A registry audience, marked by `role` on the definition (~2604 tok)
- `0022-composed-policies.md` — ADR-0022: A policy composed of others, signed as one bundle (~2712 tok)
- `0023-composed-policies.md` — ADR-0023: A policy composed of others, signed as one bundle (~2850 tok)
- `0026-bundle-schema-v4-and-its-rollout.md` — ADR-0026: Bundle schema v4 and its rollout (~2412 tok)

## docs/air/

- `catalogue.yaml` (~1582 tok)
- `mapping.yaml` — One entry per item in docs/air/catalogue.yaml. Judgments are the author's and need review by (~3068 tok)

## docs/src/adr/

- `0024-subjects-in-the-signed-bundle.md` — 8<-- "adr/0024-subjects-in-the-signed-bundle.md" (~13 tok)
- `0025-a-ceiling-shared-by-several-policies.md` — 8<-- "adr/0025-a-ceiling-shared-by-several-policies.md" (~15 tok)
- `index.md` — Architecture Decision Records (~617 tok)

## docs/src/concepts/

- `audiences.md` — SandboxAudience (~1200 tok)

## docs/src/guides/

- `consume-a-bundle.md` — Consume a bundle (~2477 tok)
- `install.md` — Install notaio (~2138 tok)
- `platform-audiences.md` — Set up platform audiences (~887 tok)

## examples/

- `knobgrant-package-install.yaml` — Relax K2.3 (package installation) from R0 to R1 for one policy, for 30 days. R1 grants last at most (~198 tok)
- `sandboxpolicy-hardened.yaml` — K8s SandboxPolicy: agent-default (~139 tok)

## scripts/

- `check_air_mapping.py` — Validate docs/air/mapping.yaml against the repo and render docs/framework-mapping.md. (~1916 tok)
