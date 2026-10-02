# GitHub Workflows & CI/CD Standards

## CRITICAL: Never Replace `firestoned/github-actions` With Direct Action Calls

ALL GitHub Actions workflows MUST use composite actions from the `firestoned/github-actions` library. NEVER replace them with direct action calls, even if the underlying action version is outdated.

**Why:** `firestoned/github-actions` is owned by the user (Erick Bourgeois). When an underlying action needs a version bump, fix it in the `firestoned/github-actions` repo, not by inlining here.

**Fix process:**
1. Update action version in the `firestoned/github-actions` repository
2. Tag a new release (e.g., v1.3.7)
3. Update the version reference in this repo's workflows

```yaml
# ✅ CORRECT
- name: Cache cargo dependencies
  uses: firestoned/github-actions/rust/cache-cargo@d0d51c638a90bffc2a1567fe7af112b37fe8854c # v1.3.7

# ❌ WRONG
- name: Cache cargo dependencies
  uses: actions/cache@v5
```

**Action families:**
- `firestoned/github-actions/rust/cache-cargo`: Cargo dependency caching
- `firestoned/github-actions/rust/setup-rust-build`: Linux cross-compilation setup
- `firestoned/github-actions/rust/build-binary`: Binary compilation
- `firestoned/github-actions/rust/generate-sbom`: SBOM generation
- `firestoned/github-actions/rust/security-scan`: Cargo audit
- `firestoned/github-actions/docker/setup-docker`: Docker login + buildx
- `firestoned/github-actions/security/license-check`: SPDX header verification
- `firestoned/github-actions/security/verify-signed-commits`: Commit signature verification
- `firestoned/github-actions/security/trivy-scan`: Container vulnerability scan
- `firestoned/github-actions/versioning/extract-version`: Image tag generation

---

## CRITICAL: All Workflows Must Be Makefile-Driven

Workflows MUST only: install tools, set env vars, and call Makefile targets. All business logic lives in the Makefile.

```yaml
# ✅ GOOD
- name: Run e2e suite ${{ matrix.suite }}
  env:
    KIND_CLUSTER_NAME: notaio-e2e-${{ matrix.suite }}
    E2E_SUITE: ${{ matrix.suite }}
  run: make kind-e2e-ci

# ❌ BAD
- name: Create cluster
  run: |
    kind create cluster --name notaio-e2e
    kubectl create namespace notaio-system
    # ... 50+ lines of bash ...
```

**Rules:**
- No multi-line bash scripts (except simple tool setup)
- All `run:` commands MUST call Makefile targets (e.g., `make clippy` not `cargo clippy ...`)
- Makefile targets MUST work identically locally and in CI
- Document targets with `## comments` for `make help`

**Exception:** a step whose whole job is a third-party checker that ships its
own binary (`EmbarkStudios/cargo-deny-action`, `rust/security-scan`, CodeQL,
Semgrep) may call the action. Keep a Makefile target for the same check where
one makes sense (`make deny`), so it still runs locally.

---

## CRITICAL: e2e Is Its Own Workflow

End-to-end tests live in `.github/workflows/e2e.yaml`, never as a job inside
`build.yaml`. It is reusable (`workflow_call`), so
`dependabot-auto-merge.yaml` gates every dependency bump on it.

- One matrix job per suite, each on its own kind cluster named
  `notaio-e2e-<suite>`, with `fail-fast: false`, so a red X names the
  contract that broke.
- Each job calls `make kind-e2e-ci E2E_SUITE=<suite>`, which dumps
  diagnostics on failure and always deletes the cluster.
- The matrix and `E2E_SUITES` in the Makefile list the same suites. Adding a
  suite means a `kind-e2e-<suite>` target, a
  `crates/notaio/tests/e2e_<suite>.rs`, a matrix row and an `E2E_SUITES` entry,
  in the same change.
- No secrets and no registry, so it runs on fork PRs.

**e2e targets:**
- `make kind-e2e`: every suite, in sequence, on one local cluster
- `make kind-e2e-<suite>`: one suite (`install`, `bundle`, `rbac`, `admission`)
- `make kind-e2e-ci`: one suite in CI (`E2E_SUITE=<suite>`)
- `make kind-e2e-logs`: controller and policy state, when a suite fails

---

## CRITICAL: Pin Every Action by Commit SHA

Every `uses:` is pinned to a full commit SHA with the version in a trailing
comment (`@<sha> # v7.0.1`). Tags move; SHAs do not (Scorecard
Pinned-Dependencies). Dependabot bumps the SHA and the comment together.
Container images in workflows are pinned by tag **and** digest.

---

## CRITICAL: Workflows Must Be Reusable and Composable

New workflows MUST support both `workflow_call` (called by other workflows) and standalone triggers.

**Reusable workflow pattern:**
```yaml
on:
  workflow_call:
    inputs:
      kind_node_image:
        required: false
        type: string
  workflow_dispatch:
    inputs:
      kind_node_image:
        required: false
        type: string

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - run: make kind-e2e-ci
        env:
          KIND_NODE_IMAGE: ${{ inputs.kind_node_image }}
```

**Calling reusable workflows:**
```yaml
jobs:
  e2e:
    needs: metadata
    uses: ./.github/workflows/e2e.yaml
```

**Checklist before adding a new workflow:**
- [ ] Can this be a job in an existing workflow?
- [ ] Is it reusable via `workflow_call`?
- [ ] Does it duplicate existing logic?
- [ ] Can it be a composite action?
