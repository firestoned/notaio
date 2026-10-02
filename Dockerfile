# Distroless image for the notaio controller.
#
# This Dockerfile expects a pre-built Linux binary at
# `binaries/<TARGETARCH>/notaio`, built by the Makefile (`make kind-load`
# builds natively on Linux, or with a gcc cross-toolchain on macOS). We never
# compile inside the container. The binary uses rustls (no OpenSSL), so this is
# a plain single-stage COPY.
#
# Today this image exists for the kind e2e (`make kind-e2e`). Publishing,
# signing and provenance are ROADMAP M2 and will reuse it.
#
# Pinned by digest for supply-chain reproducibility. Dependabot (docker
# ecosystem) opens a PR with the new digest when upstream publishes a patched
# image. Do NOT revert to a floating tag.
#
# The digest MUST sit on a literal `FROM` line. Dependabot's Docker parser
# reads `FROM` instructions and does not expand `ARG` defaults
# (dependabot/dependabot-core#4597, #10190), so a digest hidden in an
# `ARG BASE_IMAGE=...` consumed via `FROM ${BASE_IMAGE}` is invisible to it.
#
# BASE_IMAGE stays overridable for air-gapped or mirrored builds: it defaults
# to the *stage name*, so an unset override resolves to the pinned digest below
# and a set one resolves to the caller's mirror.
ARG BASE_IMAGE=pinned-base

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2 AS pinned-base

FROM ${BASE_IMAGE}

ARG VERSION
ARG GIT_SHA
ARG TARGETARCH

LABEL org.opencontainers.image.source="https://github.com/firestoned/notaio" \
      org.opencontainers.image.description="notaio: compiles sandbox policies into signed, versioned bundles" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${GIT_SHA}"

COPY --chmod=755 binaries/${TARGETARCH}/notaio /usr/local/bin/notaio

# uid/gid 65532, matching runAsUser in deploy/controller/20-deployment.yaml.
USER nonroot

ENTRYPOINT ["/usr/local/bin/notaio"]
