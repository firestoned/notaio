# Threat Modeling

> **After implementing an ADR, do a full pass over the threat model mapping.**
> It is the last step of the ADD cycle (`rules/architecture-driven-development.md`),
> and an ADR is not done until it has happened.

## What the threat model is here

notaio does not own its threat model. The **AgentSandbox Threat Model and
Security Roadmap** is the source of truth for the knob register, the profiles,
the hard floors and the threat identifiers (T5.4, T8.2, K5.3, …). This
repository makes part of it executable, and `docs/threat-model-mapping.md`
says which part:

- **Implemented here**: the model item and the file that enforces it
- **Supported, enforced elsewhere**: what notaio provides and who enforces it
- **Not covered**: what is out of scope, stated so nobody assumes otherwise

`crates/sandboxpolicy-core/src/registry.rs` and `builtin.rs` are the model's
knob register and profiles in code. `docs/air/*.yaml` and the generated
`docs/framework-mapping.md` map the same controls onto the AIR framework.

## The rule

When the implementation of an ADR is complete (code written, tests green,
CHANGELOG and docs updated), make a **full pass** over
`docs/threat-model-mapping.md` before declaring the task finished. Full pass
means every row, not appending one to the table that obviously changed: a
change that adds a controller capability can move a row from "Supported" to
"Implemented", and it can just as easily make an "Implemented" row false.

If the code and the model disagree, **say so and ask**. Do not quietly pick one
(CLAUDE.md).

## Trigger questions

Any **yes** means the mapping changes:

| Question | What to revisit |
| --- | --- |
| New or changed lint rule, ceiling or knob maximum? | "Implemented here" rows; `registry.rs` / `builtin.rs` against the model |
| New knob, or a changed step in a profile? | `registry.rs`, every built-in profile (`registry_matches_profiles`), `make manifests` |
| Change to the bundle: format, signing, version, expiry, verification? | T5.4 and the signed-bundle hard floor; ADR-0002 |
| Change to what the controller can read or write (RBAC, namespaces, Secrets)? | T8.2; `deploy/controller/10-rbac.yaml`; the `e2e_rbac` suite |
| Change to admission (`deploy/admission/`)? | T8.2, scenario S6; the `e2e_admission` suite |
| New network path, listener, or credential? | CLAUDE.md "Do not" list; `30-networkpolicy.yaml` |
| New consumer-facing contract (banlieue, mediatore, gateway)? | "Supported, enforced elsewhere" |
| Does it make a "Not covered" item covered, or the reverse? | "Not covered" |

## Requirements for the pass

1. **Every "Implemented here" row cites a control that exists**, the file that
   enforces it, the way the current rows do. Never write a control that does
   not exist yet as though it does. A gap is either fixed before the ADR counts
   as implemented, or stated honestly in the right section.
2. **Never loosen** a lint rule, a ceiling or a knob maximum without a change
   to the threat model to point to (CLAUDE.md).
3. **Keep `docs/air/` in step** and run `make air` when a control moved.
4. **Stamp the pass.** Add or bump a line under the document's title:
   `Last full pass YYYY-MM-DD, against ADR-0001 … ADR-NNNN.` The stamp is the
   deliverable: an unchanged stamp means the pass did not happen. The first
   pass under this rule adds the line.
5. **"No change" is a valid outcome**, but it is a conclusion, not a skip.
   Bump the stamp anyway and record it in `.claude/CHANGELOG.md`.
6. **Never record a specific unremediated vulnerability** in a tracked file.
   That goes to private vulnerability reporting, and any working notes stay
   outside the repository (`~/dev/roadmaps/`, per the global instructions).
7. **No real infrastructure identifiers** (`rules/no-real-infrastructure.md`).

## Scope

**Required** for any ADR that reached implementation: the same set of changes
that needed an ADR in the first place.

**Not required** for TDD-only changes. But if a "trivial" fix turns out to
change who can reach what, it was not trivial: write the ADR, then do the pass.

## Checklist

- [ ] Every row of `docs/threat-model-mapping.md` walked, not just the obvious one
- [ ] Every trigger question answered against this ADR
- [ ] `registry.rs` / `builtin.rs` still match the model; `make manifests-check` passes
- [ ] `docs/air/` updated and `make air-check` passes, if a control moved
- [ ] Stamp added or bumped: date **and** ADR range
- [ ] `.claude/CHANGELOG.md` records the pass (with `**Author:**`)
- [ ] No unremediated finding and no real infrastructure identifier committed
