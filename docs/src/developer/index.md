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
