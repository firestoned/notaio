# Changelog

Newest first. Every entry carries `**Author:**` (`update-changelog` skill).

## [2026-10-14 00:00] - Replace serde_yaml_ng with serde-saphyr; rustls 0.23.46

**Author:** Erick Bourgeois

### Changed
- `crates/notaio/Cargo.toml`: `serde_yaml_ng` replaced by `serde-saphyr` 2.0.0 (MIT OR
  Apache-2.0, passes `make deny`). Used by `crdgen` and two test files only.
- `crates/notaio/src/bin/crdgen.rs`, `crates/notaio/tests/manifests.rs`,
  `crates/notaio/tests/e2e_install.rs`: `serde_saphyr::to_string` and
  `serde_saphyr::from_multiple`.
- `deploy/crds/sandbox.firestoned.io.yaml`: regenerated. Formatting only (quote style, folded long
  descriptions); parses to the same data as before. `deploy/profiles/builtin.yaml` is byte-identical.
- `Cargo.lock`: minimal change. `serde-saphyr` and `granit-parser` in; `serde_yaml_ng`,
  `unsafe-libyaml`, `ryu` and a duplicate `hashbrown`/`indexmap` out; `rustls` 0.23.45 to 0.23.46.

### Why
RUSTSEC-2024-0449 marks `unsafe-libyaml` unmaintained (a c2rust translation of libyaml, archived,
bugs not fixed upstream), and cargo-deny and cargo-audit failed the Build on `main`. `crdgen` is the
source of truth for the checked-in manifests, so it should not depend on it. RUSTSEC-2026-0346 to
0349 (rustls, fixed in 0.23.46) were published at the same time and failed the same jobs.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout (the CRD schema is unchanged)
- [ ] Config or examples change only
- [ ] Documentation only

## [2026-10-03 00:00] - How the AgentSandbox components relate

**Author:** Erick Bourgeois

### Changed
- `docs/architecture.md`: new "Where notaio sits in AgentSandbox" section. A platform diagram, the
  components and each one's relationship to notaio, the contracts that cross component boundaries
  (who writes, who reads, where defined, state), sequence diagrams for the life of a change and the
  life of a sandbox, and who trusts what. Planned consumer behaviour is marked M3 throughout.
- `README.md`: linked banlieue, mediatore and mediatore-guest; an AgentSandbox section with the
  platform diagram; the name's meaning.
- `docs/src/index.md`: the same links, a short AgentSandbox paragraph and the platform diagram.
- `docs/mkdocs.yml`: no creation date from git-revision-date-localized, which failed strict builds
  at random on uncommitted pages.

### Why
The relationships between the AgentSandbox components were spread across ADRs in three
repositories. The core docs now explain them in one place.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [x] Documentation only

## [2026-10-02 00:00] - Badges, SECURITY.md and a MkDocs site, as in banlieue

**Author:** Erick Bourgeois

### Changed
- `README.md`: workflow badges (Build, E2E, Documentation, CodeQL, SAST), OpenSSF Scorecard, Rust,
  docs site, security policy, status, issues, last commit. The status text now says the controller
  runs end to end on kind.
- `SECURITY.md`: private vulnerability reporting, response times, notaio-specific scope.
- `docs/mkdocs.yml`, `docs/pyproject.toml`, `docs/poetry.lock`, `docs/README.md`, `docs/src/`:
  MkDocs Material site. Canonical docs (architecture, threat model mapping, framework mapping, ADRs,
  ROADMAP, built-in profiles) are pulled in with snippets, never copied. Strict build.
- `.github/workflows/docs.yaml`: builds on PRs, publishes to GitHub Pages on push to `main`.
  No CALM jobs and no release chain yet.
- `.github/requirements/poetry.{in,txt}`, `.github/tools/linkinator/`: hash-pinned Poetry and
  linkinator, from banlieue.
- `.github/dependabot.yml`: pip (`/docs`) and npm (linkinator) ecosystems.
- `Makefile`: `docs`, `docs-serve`, `docs-linkcheck`, `docs-clean`.
- `docs/architecture.md`: the first "Known gaps in v0" item now reflects the kind e2e.
- `ROADMAP.md`: M5 progress. `.claude/rules/documentation.md`: the site and its stub rule.

### Why
Same public face and supply-chain posture as banlieue.

### Impact
- [ ] Breaking change
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [x] Documentation only

## [2026-09-30 00:00] - Rename to notaio; port CI, e2e and Claude rules from banlieue

**Author:** Erick Bourgeois

### Changed
- Whole tree: the project is renamed from sicario to notaio. Crate and binary `notaio`
  (`crates/notaio`), namespace `notaio-system`, env vars `NOTAIO_*`, SSA field manager and
  `app.kubernetes.io/managed-by` label `notaio`, image and repository `firestoned/notaio`.
- `.github/workflows/build.yaml`: replaces `ci.yaml`. Makefile driven, SHA pinned, per-job path
  gating via `make ci-code-changed`, signed-commit verification, cargo-audit and cargo-deny.
- `.github/workflows/e2e.yaml`: new. One kind cluster per suite (`install`, `bundle`, `rbac`,
  `admission`), reusable via `workflow_call`.
- `.github/workflows/{codeql,sast,scorecard,dependabot-auto-merge}.yaml`, `.github/dependabot.yml`,
  `.github/CODEOWNERS`, `.github/codeql/codeql-config.yml`: ported from banlieue. CodeQL also
  analyses Python (`scripts/check_air_mapping.py`).
- `crates/notaio/tests/e2e_*.rs`, `crates/notaio/tests/common/mod.rs`: the e2e suites.
- `Makefile`: `help`, `fmt-check`, `clippy`, `deny`, `ci-code-changed`, the `kind-*` and
  `kind-e2e-*` targets.
- `Dockerfile`, `.dockerignore`: distroless image, pinned by digest, for the e2e.
- `.claude/rules/`: architecture-driven-development, threat-modeling, testing, rust-style,
  documentation, github-workflows, no-real-infrastructure, adapted to notaio.
- `.claude/skills/`: cargo-quality, regen-manifests, tdd-workflow, update-changelog.
- `CLAUDE.md`: ADD section and an index of the rules and skills.

### Why
Bring notaio under the same ADD discipline, CI shape and supply-chain posture as banlieue, and
give ROADMAP M1 ("run the controller end to end") an executable, repeatable form.

### Impact
- [x] Breaking change (namespace, env vars, field manager and label renamed)
- [ ] Requires cluster rollout
- [ ] Config or examples change only
- [ ] Documentation only
