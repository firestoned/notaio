# OpenWolf

This project uses OpenWolf for context management. The always-on rules live in `.claude/rules/openwolf.md`; the hooks handle bookkeeping (anatomy index, memory log, read tracking) automatically.

For the full operating protocol (session handoff, memory discipline, bug logging), load the `openwolf` skill, or read `.wolf/OPENWOLF.md`. Regenerate the session handoff with `/handoff`.


# Instructions for Claude Code in this repository

Read `README.md`, `docs/architecture.md`, both ADRs and `docs/threat-model-mapping.md` first. The
AgentSandbox threat model is the source of truth for the knob register and the profiles. This
repository makes it executable. If the code and the model disagree, say so and ask. Do not quietly
pick one.

## Governing methodology: Architecture Driven Development (ADD)

notaio is built ADR first. For any architecturally significant change the order is fixed:

```
ADR  →  CALM  →  TDD  →  implement  →  docs  →  threat model pass
```

ADRs, the architecture model and the threat model mapping are deliverables equal to code and tests.
An ADR is not implemented until the threat model pass is done. The CALM model is not bootstrapped
yet; until it is, the CALM step updates `docs/architecture.md`. Details and the checklist:
`.claude/rules/architecture-driven-development.md`. When unsure whether a change is architectural,
write the ADR.

## Rules and skills

- ADD: `.claude/rules/architecture-driven-development.md`
- Threat model pass after every implemented ADR: `.claude/rules/threat-modeling.md`
- TDD, test tiers, e2e: `.claude/rules/testing.md`, `tdd-workflow` skill
- After any Rust change: `cargo-quality` skill (not optional)
- CRD type, knob or profile change: `regen-manifests` skill
- Style, magic numbers, early returns: `.claude/rules/rust-style.md`
- Docs and changelog: `.claude/rules/documentation.md`, `update-changelog` skill
- Workflows are Makefile driven, SHA pinned, e2e in its own workflow: `.claude/rules/github-workflows.md`
- Public repository, no real hostnames, IPs, accounts or key ids: `.claude/rules/no-real-infrastructure.md`

## Working rules

1. Explore before editing. Do not assume a file, field or behaviour exists. Do not invent file paths,
   API fields or Kubernetes behaviour. If you are unsure, say so and ask.
2. Small, reviewable changes. One topic per branch and per pull request.
3. Run `make lint`, `make test` and `make manifests-check` before proposing a change. If you change a
   CRD type or a built-in profile, run `make manifests` and commit the result. Never hand edit
   `deploy/crds` or `deploy/profiles`.
4. Raw Kubernetes manifests only. No Helm. No HashiCorp tooling for new decisions.
5. Every new dependency must be permissive and foundation friendly. Screen for BSL, AGPL and
   single-vendor control before adding one, and run it past `deny.toml`.
6. In all prose you write (docs, comments, commit messages), do not use em-dashes. Use commas or other
   punctuation.

## Code rules

- `sandboxpolicy-core` is pure. No I/O, no clock reads, no network. Time comes in as `now`.
- `sandboxpolicy-types` and `sandboxpolicy-bundle` must never depend on `kube` or `k8s-openapi`.
  mediatore-guest runs as root in every sandbox VM and depends on the bundle crate.
- No `unwrap` or `expect` outside tests. No `unsafe`.
- Every lint rule has a stable code, a documented meaning and a test that fails when the rule is
  weakened. Add the test with the rule.
- Changing the canonical form of `BundleContent` (field order, types) is a schema change. Bump
  `SCHEMA` and update ADR-0002.

## Do not

- Do not publish an unsigned bundle, or add a code path that does.
- Do not make the controller able to create, edit or delete policy objects. Its RBAC is read plus
  status writes by design.
- Do not add hypervisor credentials, agent traffic or any inbound listener to the controller.
- Do not add policy logic to banlieue or mediatore from this repository. They consume the contracts.
- Do not loosen a lint rule, a ceiling or a knob maximum without a change to the threat model to point
  to.
- Do not add a knob to `registry.rs` without adding it to every built-in profile. The
  `registry_matches_profiles` test enforces this.

## Where to start

`ROADMAP.md`, milestone M1. The first useful piece of work is running the controller on a k0s test
cluster and reporting what breaks, before changing anything. `make kind-e2e` runs the same loop on
a local kind cluster (`.claude/rules/testing.md`).
