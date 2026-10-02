---
name: update-changelog
description: Prepend an entry to .claude/CHANGELOG.md after any code, manifest, workflow or architecture change. **Author:** is mandatory. Load at the end of a task, after the cargo-quality gate.
---

# Update the Changelog

Prepend an entry to `.claude/CHANGELOG.md`, newest first, in exactly this
shape:

```markdown
## [YYYY-MM-DD HH:MM] - Brief Title

**Author:** <Name of requester or approver>

### Changed
- `path/to/file.rs`: what changed

### Why
The technical or threat-model reason. Cite the ADR, knob or threat id.

### Impact
- [ ] Breaking change (CRD schema, bundle schema, public API)
- [ ] Requires cluster rollout (CRDs, profiles or controller)
- [ ] Config or examples change only
- [ ] Documentation only
```

## Rules

- `**Author:**` on every entry, no exceptions.
- A new dependency says why it was added and that it passed `make deny`.
- A threat model pass is recorded even when the conclusion is "no change".
- A bundle `SCHEMA` bump is called out as breaking.
- No em-dashes, no real infrastructure identifiers.
- Past entries are history: never rewrite them to match later renames.
