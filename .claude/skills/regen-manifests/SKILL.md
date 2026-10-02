---
name: regen-manifests
description: Regenerate deploy/crds/ and deploy/profiles/ from the Rust types after ANY change to a CRD type, the knob register or a built-in profile, including doc-comment-only edits, which land in the CRD descriptions. Never hand edit generated YAML. Load whenever sandboxpolicy-api, registry.rs or builtin.rs changed, or a status field silently fails to persist on a cluster.
---

# Regenerate the Manifests

The Rust types are the source of truth. `deploy/crds/sandbox.firestoned.io.yaml`
and `deploy/profiles/builtin.yaml` are **generated** by the `crdgen` binary;
never edit either by hand (CLAUDE.md).

```sh
make manifests         # crdgen crds > deploy/crds/…, crdgen profiles > deploy/profiles/…
make manifests-check   # what CI runs: fails on any drift
```

## When to run

- After **any** edit under `crates/sandboxpolicy-api/src/`: fields, serde or
  schemars attributes, **and doc comments**, which become `description:`.
- After a change to `crates/sandboxpolicy-core/src/registry.rs` or
  `builtin.rs`. A new knob goes into every built-in profile; the
  `registry_matches_profiles` test enforces it.
- When a status patch "succeeds" but the field never persists on a cluster:
  the deployed CRD predates the Rust type.

## After regenerating

1. **Diff the output** (`git diff deploy/`) and check it contains exactly the
   change you made. An unexpected diff means someone hand edited a generated
   file, or the change rippled further than intended.
2. **Update `examples/`** to match, and let `make kind-e2e-install` dry-run
   them against the live schema.
3. **Commit the generated files with the change.** A type change without its
   YAML is incomplete.
4. A profile or knob change also needs the threat model pass
   (`rules/threat-modeling.md`).

## The deployment half

Regenerating fixes the repository. A live cluster serves the old schema until
the CRD is re-applied, and keeps the old profiles until `deploy/profiles/` is
re-applied. Tell the user which changed. Diff before applying to a shared
cluster: `kubectl diff -f deploy/crds/ -f deploy/profiles/`.
