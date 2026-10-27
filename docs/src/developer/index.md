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
| `install` | the CRDs are established, the controller comes up, every built-in profile is judged Valid, and `examples/` passes a server-side dry run; the controller answers `/healthz`, `/readyz` and `/metrics`, and both must-be-zero series are 0 |
| `bundle` | a policy and a grant compile to a signed bundle a consumer verifies with the public key; a wrong key and a downgrade are both refused |
| `rbac` | the controller can read and write status and record Events, and cannot create, edit or delete policy objects, read Events back, or read Secrets |
| `admission` | the GitOps-only admission policy denies cluster-admin, admits the GitOps identity, and leaves status writes alone |

On macOS, `make kind-load` cross-compiles and needs a gcc cross toolchain
(`brew tap messense/macos-cross-toolchains && brew install x86_64-unknown-linux-gnu`, or the
`aarch64` one on Apple silicon). With rootless podman instead of Docker:

```sh
KIND_EXPERIMENTAL_PROVIDER=podman make kind-e2e CONTAINER_TOOL=podman
```

## Checking policies with `notaio lint`

```sh
make policy-lint      # this repository's examples and profiles, with a pinned clock
```

Running it in a policy repository, its exit and input codes, and the GitHub Actions job are in
[Lint a policy repository](../guides/lint-in-ci.md).

## A test OpenBao for the signer

The production signer is an OpenBao transit key (ADR-0004). `dev/openbao/` stands one up on a
test cluster: a single OpenBao 2.7 with PostgreSQL 18 as its storage backend, TLS 1.3 from a
namespaced cert-manager CA, the audit device on, and exactly what ADR-0004 asks for. That's a
non-exportable ed25519 key `notaio`, a policy that allows `update` on `transit/sign/notaio` and
nothing else, and a Kubernetes auth role for `notaio-system/notaio` (audience `openbao`, 15 minute
tokens). Only `notaio-system` can reach it.

```sh
make openbao-dev       OPENBAO_KUBECONFIG=<kubeconfig> OPENBAO_STATE=<dir outside the repo>
make openbao-dev-check OPENBAO_KUBECONFIG=<kubeconfig> OPENBAO_STATE=<same dir>
```

`openbao-dev-check` checks it from the outside, the way the signer will use it: the storage
backend, the key's type and exportability, that `notaio-system/notaio` can sign and can do
nothing else, that another ServiceAccount cannot log in, that another namespace cannot reach it,
and that the audit log records the signature. It exits non-zero on any surprise and removes what
it creates.

It needs cert-manager and a default StorageClass. The unseal key and root token go to
`$OPENBAO_STATE/init.json` (mode 600), the public key consumers trust to
`$OPENBAO_STATE/notaio-public-key.txt`. A restart of the OpenBao pod seals it; run the target
again to unseal. It is a test instance: one replica, one key share.

## Signing with OpenBao

The controller signs with an OpenBao transit key (ADR-0004), as it ships. It trusts only the CA in
ConfigMap `openbao-ca` (key `ca.crt`) and logs in with a projected ServiceAccount token, so a
cluster needs an OpenBao first; `dev/openbao` provides a test one. Every kind e2e suite runs that
way: `kind-deploy` installs cert-manager and dev/openbao, and `dev/openbao/use-signer.sh ca`
copies the CA.

```sh
make kind-e2e-openbao                       # kind: cert-manager, dev/openbao, the suite
make openbao-e2e CLUSTER_KUBECONFIG=<kubeconfig> OPENBAO_STATE=<dir> \
    NOTAIO_IMAGE=ghcr.io/firestoned/notaio@sha256:<digest>   # a test cluster with dev/openbao
```

`e2e_openbao` checks that the controller holds no development seed, that a bundle is signed by
`notaio:v<N>` and verifies against OpenBao's public key, and that with OpenBao gone nothing is
published and the policy reports `Ready=False, SigningFailed`. Both targets unseal OpenBao
afterwards.

For a local experiment without OpenBao, `dev/openbao/use-signer.sh <kubeconfig> development`
switches to a development seed (`deploy/signer/development-patch.yaml`), and `openbao` switches
back. Never in production: anyone who can read the seed's Secret can sign.

## Consuming a bundle

The consumer loop, with `sandboxpolicy_bundle::floor`, is in
[Consume a bundle](../guides/consume-a-bundle.md). To run it against a published bundle:

```sh
cargo run -p sandboxpolicy-bundle --example verify_and_record -- \
    bundle.dsse.json <key-id> <public-key-base64> floor.json
```

## Verifying a release

Images come only from this repository's Build workflow (ADR-0009): `latest`, `main-<date>` and
`sha-<sha>` from `main`, `v<X.Y.Z>` from a release. Install from a release's
`notaio-<version>.yaml`, which pins the image by digest. To check what you are about to run:

```sh
IMAGE=ghcr.io/firestoned/notaio@sha256:<digest from notaio-<version>.yaml>
ID='^https://github.com/firestoned/notaio/.github/workflows/build.yaml@refs/(heads/main|tags/v.+)$'

# The image was signed by this repository's workflow, keyless.
cosign verify "$IMAGE" --certificate-identity-regexp "$ID" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com

# It was built by this repository's CI, from a commit you can see.
gh attestation verify "oci://$IMAGE" --repo firestoned/notaio

# The install manifest itself is the one the release built.
cosign verify-blob notaio-<version>.yaml --bundle notaio-<version>.yaml.sigstore.json \
  --certificate-identity-regexp "$ID" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
sha256sum -c checksums.sha256
```

!!! note "Cosign 3 or later"
    Images are signed by Cosign 3, which stores signatures as OCI referrers in the new bundle
    format. Cosign 2 does not look there and reports `no signatures found`, which reads like an
    unsigned image. Check `cosign version` first.

!!! note "`gh attestation verify` says the token was denied"
    `gh` reads ghcr.io through your Docker credential store. A stale ghcr login there is refused
    even though the image is public. Run it with an empty Docker config, which reads anonymously:
    `DOCKER_CONFIG=$(mktemp -d) gh attestation verify ...`

The image's SBOM (`notaio-image.cdx.json` on each release) lists the base image's packages and
every Rust crate in the binary, at the version built: the binary is built with `cargo auditable`,
which embeds its dependency tree, and `make sbom-check` fails the build if the SBOM loses it.

The release logic is tested on every pull request: `make release-manifest-test` (the image is
pinned by digest exactly once, every kind is there, the admission policy is not) and
`make set-version-test`.

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
