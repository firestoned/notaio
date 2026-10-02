# Documentation Standards

## Before Marking Any Task Complete

Always ask: **does documentation need to be updated?** Code, CRD, bundle,
configuration, deployment and architecture changes all have a docs half.

---

## Where things live

| What | Where |
| --- | --- |
| What notaio is, crate map | `README.md` |
| Architecture | `docs/architecture.md` (the CALM model, once bootstrapped: `rules/architecture-driven-development.md`) |
| Decisions | `docs/adr/NNNN-title.md` |
| Threat model coverage | `docs/threat-model-mapping.md` (`rules/threat-modeling.md`) |
| AIR framework mapping | `docs/air/*.yaml`, generating `docs/framework-mapping.md` (`make air`) |
| Roadmap | `ROADMAP.md` (milestones M0 to M5) |
| Examples | `examples/*.yaml` |
| Change log | `.claude/CHANGELOG.md` |
| Generated manifests | `deploy/crds/`, `deploy/profiles/` (`make manifests`, never hand edited) |

---

## Roadmap and ADR naming

ADRs: `docs/adr/NNNN-title.md`, lowercase, hyphens only, four-digit
zero-padded, never renumbered.

The global convention for roadmaps is detail docs in `.github/community/`
(`NN-title.md`, lowercase, hyphens, two-digit prefix contiguous from `00`)
indexed by `ROADMAPS.md` at the repo root. notaio still has a single
`ROADMAP.md` with milestones. Until it moves to that layout, update the
milestone in `ROADMAP.md`; when it moves, follow the global rule, including
renumbering every `roadmap NN` reference in the same commit.

The trigger for a roadmap update is **completion, not change**: if an item is
true now, mark it now, and audit the rest of the milestone against the tree
while you are there.

---

## What to update, by change type

**CRD type** (`crates/sandboxpolicy-api/src/`):
`regen-manifests` skill, then every example using that kind, then
`README.md` if the user-visible shape changed. Doc comments land in the CRD
`description:` fields, so a comment-only edit still needs `make manifests`.

**Knob register or built-in profile** (`registry.rs`, `builtin.rs`):
`make manifests`; the threat model pass (`rules/threat-modeling.md`);
`docs/air/` if a control moved.

**Lint rule**: the rule's code and meaning in rustdoc, the test that fails
when the rule is weakened, and the "Implemented here" row it backs in
`docs/threat-model-mapping.md`.

**Bundle format or verification** (`sandboxpolicy-bundle`): ADR-0002 (and a
`SCHEMA` bump for a canonical form change), the threat model pass.

**Controller behaviour, RBAC, deployment** (`crates/notaio/`, `deploy/`):
`docs/architecture.md`, the e2e suite that covers it, and the comment block in
the manifest itself (the deploy YAML is documentation for whoever installs it).

---

## Examples must match the CRDs

Verify every field name against `deploy/crds/sandbox.firestoned.io.yaml` or
the Rust types before writing an example. Never guess. `make kind-e2e-install`
applies `examples/` with a server-side dry run, so a wrong field fails there.

---

## Changelog requirements

Every entry in `.claude/CHANGELOG.md` has `**Author:**`, no exceptions. Use
the `update-changelog` skill for the format.

---

## Code comments

Public functions and types have rustdoc (`rules/rust-style.md`). Cite the
threat model item or ADR a behaviour comes from.

---

## Prose

No em-dashes anywhere: docs, ADRs, comments, commit messages (CLAUDE.md). Use
a colon, comma, parentheses or a new sentence. Check before finishing:

```sh
rg -n '—' <files you touched>
```

---

## Validation checklist

- [ ] `.claude/CHANGELOG.md` updated with `**Author:**`
- [ ] Affected docs and ADRs updated
- [ ] Examples match the CRDs (`make kind-e2e-install` dry-runs them)
- [ ] `make manifests-check` and `make air-check` pass
- [ ] `ROADMAP.md` updated for anything that completed
- [ ] No em-dashes in what you wrote
