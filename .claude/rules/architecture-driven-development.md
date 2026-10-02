# Architecture Driven Development (ADD)

> **ADD is the governing methodology for notaio.** Architecture is decided,
> recorded and modelled **before** code is written, and its security posture is
> re-verified against the threat model **after**. ADRs, the CALM model and the
> threat model mapping are first-class deliverables, equal to code and tests.

ADD layers on top of TDD (`rules/testing.md`); it does not replace it. The
order is fixed:

```
ADR  →  CALM  →  TDD  →  implement  →  docs  →  threat model pass
```

notaio has one extra constraint banlieue does not: the **AgentSandbox threat
model is the source of truth** for the knob register and the profiles, and
this repository makes it executable. An ADR here may not contradict that model.
If the design needs the model to change, stop and ask (CLAUDE.md), and point
the ADR at the model change once it exists.

## The ADD cycle

For any **architecturally significant** change, complete each step before
starting the next.

### 1. ADR: decide and record (FIRST)

Write or update an Architecture Decision Record in `docs/adr/NNNN-title.md`
(lowercase, hyphens, four-digit zero-padded number, never renumbered).

The title line is `# ADR-NNNN: Title`, matching `0001` and `0002`. Metadata
is a bullet list under the title, one field per bullet, so status and date stay
greppable:

```markdown
# ADR-NNNN: Title

- **Status:** Accepted
- **Date:** 2026-09-30
- **Proposed:** 2026-09-29          (when it sat Proposed first)
- **Deciders:** Erick Bourgeois
- **Amended:** 2026-10-10 (Decision #3, …)
- **Supersedes:** ADR-NNNN
- **Related:** Extends [ADR-NNNN](…); threat model T8.2, K5.3
```

`Status` and `Date` are required; the rest appear only when they apply. Then
the standard sections:

- **Context**: the forces, constraints and the problem being solved
- **Decision**: what we will do, stated plainly
- **Consequences**: trade-offs, follow-ups, what this rules out

Status runs Proposed → Accepted (→ Superseded by NNNN). *Accepted* records that
the decision is made, not that it shipped; say `Not implemented.` when that is
the case. One decision per ADR. Reversing an earlier ADR marks the old one
*Superseded* and links forward.

A change to the canonical form of `BundleContent` is always an ADR-0002
amendment and a `SCHEMA` bump (CLAUDE.md, Code rules).

### 2. CALM: model and visualise

Update the FINOS CALM model (`docs/architecture/calm/architecture.json`):
nodes, relationships, interfaces, controls and flows. Then:

```sh
make calm-validate     # the model conforms to the meta-schema (hard gate)
make calm-diagrams     # regenerate the Mermaid diagrams
```

A change that is not reflected in CALM is not designed yet.

> **Not bootstrapped yet.** notaio has no CALM model, no `calm-*` targets and
> no CALM workflow; `docs/architecture.md` is the current architecture record.
> Bootstrapping the model (and porting banlieue's `calm.yaml` and
> `calm-test.yaml` with it) is itself an ADD change with its own ADR. Until it
> lands, do this step in `docs/architecture.md` and say in the ADR that the
> CALM step was deferred.

### 3. TDD: red, green, refactor

Only now write code, tests first, per `rules/testing.md` and the
`tdd-workflow` skill. After any `.rs` change, run the `cargo-quality` skill. A
new lint rule lands with the test that fails when the rule is weakened
(CLAUDE.md).

### 4. Docs, including the roadmap

Update `.claude/CHANGELOG.md` (with `**Author:**`), `README.md`,
`docs/architecture.md` and `examples/` as affected, per
`rules/documentation.md`. A CRD type or built-in profile change runs
`make manifests` (the `regen-manifests` skill) and commits the result.

**If the work advanced a roadmap item, update the roadmap in this commit.**
The trigger is completion, not change: if an item is true now, mark it now,
even when an earlier session did the work. While you are in there, audit the
rest of it against the tree. "Done", "superseded" and "still open" are three
different answers and only the tree knows which applies.

### 5. Threat model pass (LAST)

Once the ADR is implemented, make a **full pass** over
`docs/threat-model-mapping.md` per `rules/threat-modeling.md`: every item that
is implemented here, supported elsewhere, or not covered, checked against the
code as it now is. Keep `registry.rs` and `builtin.rs` in step with the model.

**An ADR is not implemented until this pass is done.**

## When does ADD apply?

**Full ADR + CALM + threat model pass** (architecturally significant):

- New CRDs, controllers, binaries or crates
- A change to a contract: the CRD types, the bundle format (ADR-0002), the
  version and expiry rules, the published ConfigMap shape consumers read
- A change to what the controller can reach: RBAC, network, admission, the
  signer and its key custody
- A new knob, lint rule, ceiling or profile semantics
- Any decision where "why A over B" is worth recording

**TDD only** (no ADR or CALM):

- Typos, comment and doc tweaks, formatting
- Isolated bug fixes with no architectural impact
- Mechanical refactors that preserve behaviour and structure

> When unsure whether a change is architectural, **write the ADR.** A short,
> slightly redundant ADR costs little; an undocumented decision costs the next
> person a re-derivation.

## Checklist (paste into the work)

- [ ] ADR written or updated in `docs/adr/NNNN-*.md`, metadata bullets, then
      Context / Decision / Consequences
- [ ] CALM model updated and `make calm-validate` passes (or, until CALM is
      bootstrapped, `docs/architecture.md` updated and the deferral noted)
- [ ] Tests written **first**, then implementation
- [ ] `cargo-quality` passes; `make manifests-check` passes
- [ ] CHANGELOG, docs and examples updated
- [ ] Roadmap updated for anything that completed, rest of it audited
- [ ] Full threat model pass done (`rules/threat-modeling.md`)
