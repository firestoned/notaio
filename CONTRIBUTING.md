# Contributing to notaio

Thank you for helping. This page covers what every contribution needs, how a change moves from
idea to merge, and how to build and test it.

By taking part you agree to follow the
[Code of Conduct](https://github.com/firestoned/notaio/blob/main/CODE_OF_CONDUCT.md). Security
issues do **not** go in public issues or pull requests: report them privately as described in
[SECURITY.md](https://github.com/firestoned/notaio/blob/main/SECURITY.md).

## Every commit: sign-off and signature

notaio uses the [Developer Certificate of Origin](https://developercertificate.org/) (DCO) instead
of a CLA. A sign-off certifies that you wrote the change, or otherwise have the right to submit it
under the project's [Apache-2.0 licence](https://github.com/firestoned/notaio/blob/main/LICENSE).

**Every commit needs both:**

1. **A `Signed-off-by:` trailer** matching the commit author (`git commit -s`).
2. **A cryptographic signature**, GPG or SSH (`git commit -S`). CI verifies signatures on every
   pull request, and a pull request with an unsigned commit cannot merge.

```sh
git commit -s -S -m "Describe the change"
```

To fix a branch where commits lack either:

```sh
git rebase --signoff -S origin/main
git push --force-with-lease
```

## How a change gets in

notaio is built **ADR first**, under
[Architecture Driven Development](https://github.com/firestoned/notaio/blob/main/.claude/rules/architecture-driven-development.md):

```text
ADR  →  TDD  →  implement  →  docs  →  threat model
```

- **Architecturally significant changes** (a new kind, a change to the bundle contract, a new
  listener, credential or permission for the controller, a new dependency of weight) start with an
  ADR in [`docs/adr/`](https://github.com/firestoned/notaio/tree/main/docs/adr). Open it as its own
  pull request, or as the first commit of the change, so the decision is discussed before the code.
- **Everything else** needs tests, written first.
- **The threat model is the source of truth** for the knob register and the profiles. If this code
  and the model disagree, say so in the issue or pull request rather than picking one. Never loosen
  a lint rule, a ceiling or a knob maximum without a change to the threat model to point to.

Keep one topic per pull request. Say what changed, why, and how you tested it, and add an entry to
the Unreleased section of [CHANGELOG.md](https://github.com/firestoned/notaio/blob/main/CHANGELOG.md)
for anything a user would notice.

### Rules worth knowing before you write code

- **`sandboxpolicy-core` is pure.** No I/O, no clock reads, no network: time comes in as `now`.
- **`sandboxpolicy-types` and `sandboxpolicy-bundle` never depend on `kube` or `k8s-openapi`.**
  mediatore-guest runs as root in every sandbox VM and depends on the bundle crate.
- **No `unwrap` or `expect` outside tests, and no `unsafe`.**
- **Every lint rule has a stable code, a documented meaning, and a test that fails when the rule
  is weakened.** Add the test with the rule.
- **Changing the canonical form of the bundle content is a schema change.** Bump `SCHEMA` and
  update ADR-0002.
- **The controller never gets more power.** Its RBAC is read on the policy kinds, status writes,
  Events, its own leader Lease and bundle ConfigMaps; it never creates, edits or deletes a policy
  object, and it never publishes an unsigned bundle.
- **Generated files are never edited by hand.** `deploy/crds/`, `deploy/profiles/` and
  `docs/src/reference/` come from the code: `make manifests`.
- **Raw Kubernetes manifests only**, no Helm. **New dependencies** must be permissively licensed
  and pass `cargo deny check`.
- **Never commit real infrastructure identifiers**: no real hostnames, IPs or account ids, anywhere.
  See [`no-real-infrastructure.md`](https://github.com/firestoned/notaio/blob/main/.claude/rules/no-real-infrastructure.md).
- **No em-dashes in prose** you write: docs, comments, commit messages.

## Building and testing

The gate every change passes, locally and in CI:

```sh
make lint                   # rustfmt and clippy -D warnings
make test                   # every workspace test
make manifests-check        # deploy/ matches the Rust types
make docs-reference-check   # docs/src/reference matches the code
```

| Target | What it does |
| --- | --- |
| `make manifests` | Regenerate `deploy/crds/`, `deploy/profiles/` and `docs/src/reference/` |
| `make policy-lint` | `notaio lint` on the examples and the built-in profiles |
| `make kind-e2e` | Every end-to-end suite on a kind cluster, with a test OpenBao |
| `make docs` / `make docs-serve` | Build or serve the documentation site |

The e2e suites, the test OpenBao and the release checks are described in the
[developer guide](https://firestoned.github.io/notaio/developer/).

## Getting help

Open a [GitHub issue](https://github.com/firestoned/notaio/issues) for bugs and feature requests,
or a draft pull request to discuss a change in code. How decisions are made, and how to become a
maintainer, is in [GOVERNANCE.md](https://github.com/firestoned/notaio/blob/main/GOVERNANCE.md).
