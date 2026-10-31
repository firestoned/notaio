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
| Roadmap | `ROADMAPS.md` (milestones M0 to M5, the status index), detail plans in `.github/community/NN-title.md` |
| Examples | `examples/*.yaml` |
| Change log | `.claude/CHANGELOG.md` (detailed, for maintainers) |
| Release notes | `CHANGELOG.md` (Keep a Changelog: an Unreleased entry for anything a user would notice) |
| Licence headers | every tracked `.rs`, `.sh`, `.py`, Makefile, Dockerfile and `.github` workflow carries `SPDX-License-Identifier: Apache-2.0` in its first 10 lines (`make license-check`, in CI) |
| Governance | `GOVERNANCE.md`, `MAINTAINERS.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md` (the Contributor Covenant 2.1, verbatim but for the contact) |
| Generated manifests | `deploy/crds/`, `deploy/profiles/` (`make manifests`, never hand edited) |
| Generated reference | `docs/src/reference/` (`make docs-reference`, run by `make manifests`; never hand edited; `docs-reference-check` in CI) |
| Security policy | `SECURITY.md` |
| Documentation site | `docs/mkdocs.yml`, `docs/src/` (`make docs`, published to GitHub Pages from `main`) |

### The documentation site

The site under `docs/src/` does not copy the documents above: it pulls them in
with `pymdownx.snippets` stubs (`--8<-- "architecture.md"`), so each exists
once. Edit the canonical file, never the stub. A new ADR needs a stub in
`docs/src/adr/`, a row in `docs/src/adr/index.md` and a `nav` entry in
`docs/mkdocs.yml`. A new canonical doc needs the same. `make docs` is
`--strict`: a broken link, a missing nav target or an unresolved include fails
the build, and the Documentation workflow runs it on every PR that touches
the docs.

---

## Roadmap and ADR naming

ADRs: `docs/adr/NNNN-title.md`, lowercase, hyphens only, four-digit
zero-padded, never renumbered.

The global convention for roadmaps is detail docs in `.github/community/`
(`NN-title.md`, lowercase, hyphens, two-digit prefix contiguous from `00`)
indexed by `ROADMAPS.md` at the repo root. notaio follows it: `ROADMAPS.md` holds the milestones
and their status, and a milestone big enough to plan gets a detail document in
`.github/community/`, linked from its milestone. Renumber every `roadmap NN` reference in the same
commit when one is inserted or retired.

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

**Lint rule**: the rule's code and meaning in rustdoc and in `RULES`, the test
that fails when the rule is weakened, the "Implemented here" row it backs in
`docs/threat-model-mapping.md`, and `make docs-reference`.

**Metric, CLI flag or environment variable**: `make docs-reference`. A new
`NOTAIO_*` variable goes into `notaio::reference::environment_variables`; the
`every_variable_read_is_documented` test fails until it does.

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
- [ ] `make docs` passes (strict) if anything under `docs/`, `ROADMAPS.md` or `deploy/profiles/` changed
- [ ] `ROADMAPS.md` (and the roadmap in `.github/community/`) updated for anything that completed
- [ ] No em-dashes in what you wrote
