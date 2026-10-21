# Local Development

Rust 1.89 or newer. Every target below is the same one CI runs, so green locally is green in CI.

## Build, lint, test

```sh
make help              # every target, with a one-line description
make lint              # rustfmt check + clippy, warnings denied
make test              # unit and integration tests, offline
make manifests         # regenerate deploy/crds/ and deploy/profiles/ from the Rust types
make manifests-check   # fail if those generated files drifted
make air-check         # fail if docs/framework-mapping.md drifted from docs/air/
make deny              # licences, advisories and sources (deny.toml)
```

`deploy/crds/` and `deploy/profiles/` are generated. Never edit them by hand.

## End-to-end tests on kind

The e2e suites deploy notaio from `deploy/` (real image, RBAC, NetworkPolicy, restricted Pod Security)
into a local kind cluster and check it the way a consumer would. They need `kind`, `kubectl` and a
container runtime.

```sh
make kind-e2e                  # every suite, in sequence, on one cluster
make kind-e2e-install          # one suite
make kind-e2e-logs             # controller and policy state, when a suite fails
make kind-delete               # remove the cluster
```

| Suite | Proves |
| --- | --- |
| `install` | the CRDs are established, the controller comes up, every built-in profile is judged Valid, and `examples/` passes a server-side dry run |
| `bundle` | a policy and a grant compile to a signed bundle a consumer verifies with the public key; a wrong key and a downgrade are both refused |
| `rbac` | the controller can read and write status, and cannot create, edit or delete policy objects or read Secrets |
| `admission` | the GitOps-only admission policy denies cluster-admin, admits the GitOps identity, and leaves status writes alone |

On macOS, `make kind-load` cross-compiles and needs a gcc cross toolchain
(`brew tap messense/macos-cross-toolchains && brew install x86_64-unknown-linux-gnu`, or the
`aarch64` one on Apple silicon). With rootless podman instead of Docker:

```sh
KIND_EXPERIMENTAL_PROVIDER=podman make kind-e2e CONTAINER_TOOL=podman
```

## Checking policies with `notaio lint`

`notaio lint` runs the controller's checks on files, for pull requests to a policy repository
([ADR-0006](../adr/0006-notaio-lint-cli.md)):

```sh
notaio lint policies/                          # text; exit 1 on any error finding
notaio lint --format sarif policies/ > notaio.sarif
notaio lint --now 2026-10-20T00:00:00Z examples/   # pin the clock for a reproducible run
make policy-lint                               # this repository's examples and profiles
```

Exit codes: 0 clean (warnings allowed), 1 an error finding, 2 unreadable input. Findings carry the
lint codes L001 to L016 and the input codes I001 to I008, listed in `crates/notaio/src/lint.rs`.

A policy repository can run the published image on every pull request and show the findings
inline. Pin the image by digest:

```yaml
name: Policy lint
on:
  pull_request:
permissions:
  contents: read
jobs:
  lint:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - name: notaio lint
        run: |
          docker run --rm -v "$PWD:/repo:ro" -w /repo \
            ghcr.io/firestoned/notaio@sha256:<digest> \
            lint --format sarif . > notaio.sarif
      - name: Upload findings
        if: always()
        uses: github/codeql-action/upload-sarif@1c5b675653bb5c22dbe9b12b556ec555138e09fd # v4.38.1
        with:
          sarif_file: notaio.sarif
          category: notaio
```

The upload runs even when the lint fails, so the findings appear on the pull request while the
lint step keeps it red.

## Documentation

```sh
make docs         # build the site into docs/site/, strict
make docs-serve   # live reload at http://127.0.0.1:8000
```

See `docs/README.md` for how the site pulls in the canonical documents.

## How changes are made

notaio is built ADR first. An architecturally significant change starts with an ADR in `docs/adr/`,
then tests, then code, and ends with a pass over the
[Threat Model Mapping](../security/threat-model-mapping.md). The full rules are in the repository's
`CLAUDE.md` and `.claude/rules/`.
