.DEFAULT_GOAL := help

BINARY           ?= notaio
NAMESPACE        ?= notaio-system
REGISTRY         ?= ghcr.io
ORG              ?= firestoned
CONTAINER_TOOL   ?= docker
VERSION          ?= $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
GIT_SHA          ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo unknown)
RUST_LOG         ?= info

.PHONY: help build test lint fmt fmt-check clippy deny manifests manifests-check air air-check \
        ci-code-changed kind-install kind-create kind-delete kind-kubeconfig kind-load \
        kind-deploy kind-e2e kind-e2e-ci kind-e2e-logs \
        kind-e2e-install kind-e2e-bundle kind-e2e-rbac kind-e2e-admission kind-e2e-openbao \
        kind-cert-manager kind-openbao cluster-deploy cluster-undeploy openbao-e2e \
        docs docs-serve docs-clean docs-linkcheck docs-reference docs-reference-check policy-lint openbao-dev openbao-dev-check \
        release-binary release-manifest release-manifest-test set-version set-version-test

help: ## Show this help
	@echo 'Usage: make [target] [VAR=value ...]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z0-9_.-]+:.*## / {printf "  %-24s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ''
	@echo 'Common variables:'
	@echo '  KIND_CLUSTER_NAME=<name>   (default: $(KIND_CLUSTER_NAME))'
	@echo '  KIND_NODE_IMAGE=<image>    (default: $(KIND_NODE_IMAGE))'
	@echo '  E2E_SUITE=<suite|all>      (kind-e2e-ci only; one of: $(E2E_SUITES))'

# ----- build, test, lint -----------------------------------------------------

build: ## Build every workspace crate
	cargo build --workspace

test: ## Run the workspace tests (the #[ignore]d e2e suites excluded)
	cargo test --workspace

fmt: ## Format every crate
	cargo fmt --all

fmt-check: ## Fail if any crate is not formatted
	cargo fmt --all -- --check

clippy: ## Clippy over every target, warnings are errors
	cargo clippy --workspace --all-targets -- -D warnings

lint: fmt-check clippy ## fmt-check + clippy

deny: ## cargo-deny: licences, advisories, sources (deny.toml)
	cargo deny check

# deploy/crds and deploy/profiles are generated, never hand edited.
manifests: ## Regenerate deploy/crds, deploy/profiles and docs/src/reference from the Rust types
	cargo run -q -p notaio --bin crdgen -- crds > deploy/crds/sandbox.firestoned.io.yaml
	cargo run -q -p notaio --bin crdgen -- profiles > deploy/profiles/builtin.yaml
	@$(MAKE) --no-print-directory docs-reference

docs-reference: ## Regenerate docs/src/reference/ from the code (lint rules, knobs, profiles, CRDs, metrics, CLI, environment)
	cargo run -q -p notaio --bin docgen -- docs/src/reference

docs-reference-check: ## Fail if a page in docs/src/reference/ drifted from the code
	@tmp=$$(mktemp -d); trap 'rm -rf "$$tmp"' EXIT; \
	  cargo run -q -p notaio --bin docgen -- "$$tmp" && \
	  for f in "$$tmp"/*.md; do \
	    diff -u "docs/src/reference/$$(basename "$$f")" "$$f" || { echo "docs/src/reference is stale: run make docs-reference"; exit 1; }; \
	  done

manifests-check: ## Fail if deploy/crds or deploy/profiles drifted from the Rust types
	@cargo run -q -p notaio --bin crdgen -- crds | diff -u deploy/crds/sandbox.firestoned.io.yaml -
	@cargo run -q -p notaio --bin crdgen -- profiles | diff -u deploy/profiles/builtin.yaml -

# The examples carry fixed expiry dates, and a grant's lifetime is capped from `now` (L012), so
# against the real clock they go stale on a known day. Pin the clock to when they were written;
# a policy repository runs `notaio lint` without --now (ADR-0006, decision 5).
EXAMPLES_NOW ?= 2026-10-20T00:00:00Z

policy-lint: ## Run `notaio lint` on examples/ and deploy/profiles/ (dogfooding ADR-0006)
	cargo run -q -p notaio --bin notaio -- lint --now $(EXAMPLES_NOW) examples/ deploy/profiles/

# docs/framework-mapping.md is generated from docs/air/*.yaml. Edit the YAML, then run `make air`.
air: ## Regenerate docs/framework-mapping.md from docs/air/*.yaml
	python3 scripts/check_air_mapping.py

air-check: ## Fail if docs/framework-mapping.md drifted from docs/air/*.yaml
	python3 scripts/check_air_mapping.py --check

# ----- docs ------------------------------------------------------------------
#
# MkDocs Material, built with Poetry from docs/pyproject.toml. The site pulls the
# canonical documents in with snippets (docs/README.md), so `make air` and
# `make manifests` must have run first for the published pages to be current.

POETRY_HINT = Poetry not found. Install: curl -sSL https://install.python-poetry.org | python3 -

docs: ## Build the documentation site into docs/site/ (strict)
	@command -v poetry >/dev/null 2>&1 || { echo "Error: $(POETRY_HINT)"; exit 1; }
	@cd docs && poetry install --no-interaction --quiet
	@cd docs && poetry run mkdocs build --strict
	@echo "✓ Documentation built at docs/site/index.html"

docs-serve: ## Serve the documentation with live reload at http://127.0.0.1:8000
	@command -v poetry >/dev/null 2>&1 || { echo "Error: $(POETRY_HINT)"; exit 1; }
	@cd docs && poetry install --no-interaction --quiet
	@cd docs && poetry run mkdocs serve --livereload

docs-linkcheck: ## Check docs/site/ for broken links (run `make docs` first)
	@npm --prefix .github/tools/linkinator ci --silent
	@# The site's own absolute URLs are canonical tags, which 404 for any page not yet deployed.
	@# Every relative link between pages is still checked, as is every external one.
	@.github/tools/linkinator/node_modules/.bin/linkinator docs/site/ --recurse --verbosity error \
		--skip '^https://firestoned\.github\.io/notaio/'

docs-clean: ## Remove docs/site/ and docs/.venv/
	@rm -rf docs/site/ docs/.venv/
	@echo "✓ Documentation artefacts cleaned"

# ----- test OpenBao ----------------------------------------------------------
#
# A single-node OpenBao with PostgreSQL storage, for developing the transit signer (ADR-0004)
# on a long-lived test cluster. Nothing defaults: name the cluster's kubeconfig and a state
# directory outside the repository, which receives the unseal key and root token (mode 600).
# Re-run after any restart of the openbao pod, which seals it.
OPENBAO_KUBECONFIG ?=
OPENBAO_STATE ?=

openbao-dev: ## Stand up, or unseal again, the test OpenBao (OPENBAO_KUBECONFIG=... OPENBAO_STATE=...)
	@test -n "$(OPENBAO_KUBECONFIG)" || { echo "set OPENBAO_KUBECONFIG to the test cluster's kubeconfig"; exit 1; }
	@test -n "$(OPENBAO_STATE)" || { echo "set OPENBAO_STATE to a directory outside the repository"; exit 1; }
	dev/openbao/setup.sh "$(OPENBAO_KUBECONFIG)" "$(OPENBAO_STATE)"

openbao-dev-check: ## Check the test OpenBao is what ADR-0004 asks for (OPENBAO_KUBECONFIG=... OPENBAO_STATE=...)
	@test -n "$(OPENBAO_KUBECONFIG)" || { echo "set OPENBAO_KUBECONFIG to the test cluster's kubeconfig"; exit 1; }
	@test -n "$(OPENBAO_STATE)" || { echo "set OPENBAO_STATE to the directory setup.sh wrote"; exit 1; }
	dev/openbao/check.sh "$(OPENBAO_KUBECONFIG)" "$(OPENBAO_STATE)"

# ----- release (ADR-0009) ------------------------------------------------------
#
# Tests first: each piece of release logic has a target that proves it on scratch
# copies, and the Build workflow runs them on every pull request, so the
# release-only path is not exercised for the first time on a release.

VERSION_FILE  ?= Cargo.toml
RELEASE_DIR   ?= $(CURDIR)/dist
IMAGE_DIGEST  ?=
RELEASE_IMAGE  = ghcr.io/$(ORG)/$(BINARY)

release-binary: ## Release build of notaio for this host's architecture (CI runs it on native runners)
	cargo build --locked --release -p notaio --bin notaio

release-manifest: ## Render RELEASE_DIR/notaio-VERSION.yaml: CRDs, controller, profiles, image pinned to IMAGE_DIGEST
	@test -n "$(VERSION)" || { echo "set VERSION, e.g. 1.2.3"; exit 1; }
	@echo "$(IMAGE_DIGEST)" | grep -Eq '^sha256:[0-9a-f]{64}$$' || { echo "set IMAGE_DIGEST to sha256:<64 hex>, got '$(IMAGE_DIGEST)'"; exit 1; }
	@mkdir -p "$(RELEASE_DIR)"
	@out="$(RELEASE_DIR)/notaio-$(VERSION).yaml"; \
	  { printf '# notaio %s: CRDs, controller and built-in profiles, image pinned by digest (ADR-0009).\n' "$(VERSION)"; \
	    printf '# Not included: deploy/admission/gitops-only.yaml, which names your GitOps identity.\n'; \
	    printf '# Needs OpenBao and ConfigMap openbao-ca in notaio-system before the controller starts (ADR-0004).\n'; \
	    for f in deploy/crds/*.yaml deploy/controller/*.yaml deploy/profiles/*.yaml; do \
	      printf -- '---\n# Source: %s\n' "$$f"; cat "$$f"; \
	    done; \
	  } | sed -E 's#(image: )$(RELEASE_IMAGE):[^[:space:]]+#\1$(RELEASE_IMAGE)@$(IMAGE_DIGEST)#' > "$$out"; \
	  echo "✓ $$out ($(RELEASE_IMAGE)@$(IMAGE_DIGEST))"

set-version: ## Set [workspace.package] version in VERSION_FILE to VERSION; fails if the file does not then say so
	@test -n "$(VERSION)" || { echo "set VERSION, e.g. make set-version VERSION=1.2.3"; exit 1; }
	@perl -i -pe 's/^version = ".*"/version = "$(VERSION)"/ if /^\[workspace\.package\]/ ... /^\[/' $(VERSION_FILE)
	@got=$$(awk -F'"' '/^\[workspace\.package\]/ { p = 1; next } /^\[/ { p = 0 } p && /^version *=/ { print $$2; exit }' $(VERSION_FILE)); \
	  if [ "$$got" != "$(VERSION)" ]; then echo "$(VERSION_FILE): workspace version is '$$got', expected '$(VERSION)'"; exit 1; fi; \
	  echo "$(VERSION_FILE): workspace version $(VERSION)"

release-manifest-test: ## Prove release-manifest pins the image by digest, once, and keeps every kind
	@tmp=$$(mktemp -d); trap 'rm -rf "$$tmp"' EXIT; \
	  digest=sha256:$$(printf 'release-manifest-test' | shasum -a 256 | cut -d' ' -f1); \
	  $(MAKE) --no-print-directory -s release-manifest VERSION=0.0.0-test \
	    IMAGE_DIGEST=$$digest RELEASE_DIR="$$tmp" >/dev/null && \
	  out="$$tmp/notaio-0.0.0-test.yaml" && \
	  python3 scripts/check_release_manifest.py "$$out" "ghcr.io/$(ORG)/$(BINARY)@$$digest" && \
	  echo "release-manifest-test: ok"

set-version-test: ## Prove set-version changes the workspace version, and only it, on a scratch copy
	@tmp=$$(mktemp -d); trap 'rm -rf "$$tmp"' EXIT; \
	  cp Cargo.toml "$$tmp/Cargo.toml"; \
	  $(MAKE) --no-print-directory -s set-version VERSION=0.0.0-set-version-test VERSION_FILE="$$tmp/Cargo.toml" >/dev/null && \
	  changed=$$(diff Cargo.toml "$$tmp/Cargo.toml" | grep -c '^>'); \
	  if [ "$$changed" != "1" ]; then echo "set-version changed $$changed lines, expected exactly 1"; exit 1; fi; \
	  grep -q '^version = "0.0.0-set-version-test"' "$$tmp/Cargo.toml" || { echo "set-version did not set the version"; exit 1; }; \
	  echo "set-version-test: ok"

# ----- CI gating -------------------------------------------------------------
#
# The build workflow has no workflow-level `paths:` filter: a workflow that never
# triggers reports no status at all, so a required check would stay pending
# forever. The path filter lives here instead and the `changes` job applies it
# per job, so a docs-only PR skips the expensive jobs and still reports.
CI_CODE_PATHS_RE ?= ^(crates/|deploy/|docs/air/|scripts/|Cargo\.toml$$|Cargo\.lock$$|deny\.toml$$|clippy\.toml$$|Makefile$$|Dockerfile|\.github/workflows/build\.yaml$$|\.github/actions/)

ci-code-changed: ## Print true/false: does the diff against BASE_REF touch build-relevant paths?
	@base="$${BASE_REF:-origin/main}"; \
	if ! git rev-parse --verify --quiet "$$base" >/dev/null; then \
	  echo "true"; \
	  exit 0; \
	fi; \
	if ! changed="$$(git diff --name-only "$$base...HEAD" 2>/dev/null)"; then \
	  echo "true"; \
	  exit 0; \
	fi; \
	if printf '%s\n' "$$changed" | grep -Eq '$(CI_CODE_PATHS_RE)'; then \
	  echo "true"; \
	else \
	  echo "false"; \
	fi

# ----- kind ------------------------------------------------------------------

KIND_VERSION       ?= 0.24.0
KIND_CLUSTER_NAME  ?= notaio-dev
KIND_NODE_IMAGE    ?= kindest/node:v1.31.0
KIND_IMAGE          = $(REGISTRY)/$(ORG)/$(BINARY):local-dev

# A kubeconfig scoped to the kind cluster, so nothing here depends on (or
# mutates) whichever context you have selected. Gitignored.
KIND_KUBECONFIG = $(CURDIR)/.kind-kubeconfig-$(KIND_CLUSTER_NAME)

# Every kind-targeting kubectl goes through this. Passing --kubeconfig
# explicitly is what keeps `kind create cluster` from writing its context into
# your own kubeconfig as a side effect of running the e2e.
KIND_KUBECTL = kubectl --kubeconfig $(KIND_KUBECONFIG) --context kind-$(KIND_CLUSTER_NAME)

kind-install: ## Install the kind CLI if missing (checksum verified)
	@if command -v kind >/dev/null 2>&1; then \
		echo "✓ kind already installed: $$(kind version)"; \
	else \
		echo "Installing kind v$(KIND_VERSION)..."; \
		OS=$$(uname -s | tr '[:upper:]' '[:lower:]'); \
		ARCH=$$(uname -m); \
		case "$$ARCH" in x86_64) ARCH=amd64 ;; aarch64|arm64) ARCH=arm64 ;; esac; \
		BIN="kind-$${OS}-$${ARCH}"; \
		BASE_URL="https://github.com/kubernetes-sigs/kind/releases/download/v$(KIND_VERSION)"; \
		TMP=$$(mktemp -d); \
		curl -sSLf -o $$TMP/$$BIN "$$BASE_URL/$$BIN"; \
		curl -sSLf -o $$TMP/$$BIN.sha256sum "$$BASE_URL/$$BIN.sha256sum"; \
		EXPECTED=$$(awk '{print $$1}' $$TMP/$$BIN.sha256sum); \
		if command -v sha256sum >/dev/null 2>&1; then \
			ACTUAL=$$(sha256sum $$TMP/$$BIN | awk '{print $$1}'); \
		else \
			ACTUAL=$$(shasum -a 256 $$TMP/$$BIN | awk '{print $$1}'); \
		fi; \
		if [ "$$EXPECTED" != "$$ACTUAL" ]; then \
			echo "ERROR: kind checksum mismatch"; exit 1; \
		fi; \
		chmod +x $$TMP/$$BIN; \
		sudo mv $$TMP/$$BIN /usr/local/bin/kind; \
		rm -rf $$TMP; \
		echo "✓ kind v$(KIND_VERSION) installed"; \
	fi
	@command -v kubectl >/dev/null 2>&1 || { echo "ERROR: kubectl not found on PATH"; exit 1; }

kind-create: kind-install ## Create the kind cluster $(KIND_CLUSTER_NAME) if missing
	@if kind get clusters 2>/dev/null | grep -qx $(KIND_CLUSTER_NAME); then \
		echo "✓ kind cluster '$(KIND_CLUSTER_NAME)' already exists"; \
	else \
		echo "Creating kind cluster '$(KIND_CLUSTER_NAME)'..."; \
		KUBECONFIG=$(KIND_KUBECONFIG) kind create cluster --name $(KIND_CLUSTER_NAME) --image $(KIND_NODE_IMAGE) --wait 120s; \
	fi
	@kind get kubeconfig --name $(KIND_CLUSTER_NAME) > $(KIND_KUBECONFIG)
	@$(KIND_KUBECTL) cluster-info

kind-kubeconfig: kind-create ## Write a kind-scoped kubeconfig to $(KIND_KUBECONFIG)
	@kind get kubeconfig --name $(KIND_CLUSTER_NAME) > $(KIND_KUBECONFIG)
	@echo "✓ wrote $(KIND_KUBECONFIG)"

kind-delete: ## Delete the kind cluster $(KIND_CLUSTER_NAME)
	@if kind get clusters 2>/dev/null | grep -qx $(KIND_CLUSTER_NAME); then \
		kind delete cluster --name $(KIND_CLUSTER_NAME); \
	else \
		echo "✓ no cluster named '$(KIND_CLUSTER_NAME)', nothing to delete"; \
	fi

kind-load: kind-create ## Build the notaio image for the host arch and side-load it into kind
	@HOST_ARCH=$$(uname -m); \
	case "$$HOST_ARCH" in \
		arm64|aarch64) TRIPLE=aarch64-unknown-linux-gnu; ARCH=arm64; LINKER=aarch64-linux-gnu-gcc ;; \
		x86_64|amd64)  TRIPLE=x86_64-unknown-linux-gnu;  ARCH=amd64; LINKER=x86_64-linux-gnu-gcc ;; \
		*) echo "ERROR: unsupported host arch: $$HOST_ARCH"; exit 1 ;; \
	esac; \
	if [ "$$(uname -s)" = "Linux" ]; then \
		echo "Building $(BINARY) natively for $$TRIPLE..."; \
		cargo build --release --target $$TRIPLE -p $(BINARY); \
	else \
		if ! command -v $$LINKER >/dev/null 2>&1; then \
			echo "ERROR: cross-toolchain '$$LINKER' not found."; \
			echo "  macOS: brew tap messense/macos-cross-toolchains && brew install $$TRIPLE"; \
			exit 1; \
		fi; \
		echo "Cross-compiling $(BINARY) for $$TRIPLE..."; \
		rustup target add $$TRIPLE >/dev/null 2>&1 || true; \
		TRIPLE_ENV=$$(echo $$TRIPLE | tr 'a-z-' 'A-Z_'); \
		TRIPLE_US=$$(echo $$TRIPLE | tr '-' '_'); \
		env CARGO_TARGET_$${TRIPLE_ENV}_LINKER=$$LINKER \
			CC_$${TRIPLE_US}=$$LINKER \
			AR_$${TRIPLE_US}=$${LINKER%-gcc}-ar \
			cargo build --release --target $$TRIPLE -p $(BINARY); \
	fi; \
	mkdir -p binaries/$$ARCH; \
	cp target/$$TRIPLE/release/$(BINARY) binaries/$$ARCH/; \
	echo "Building image $(KIND_IMAGE) (linux/$$ARCH)..."; \
	$(CONTAINER_TOOL) build \
		--build-arg TARGETARCH=$$ARCH \
		--build-arg VERSION="$(VERSION)" \
		--build-arg GIT_SHA="$(GIT_SHA)" \
		-t $(KIND_IMAGE) -f Dockerfile .; \
	echo "Loading $(KIND_IMAGE) into kind cluster '$(KIND_CLUSTER_NAME)'..."; \
	kind load docker-image $(KIND_IMAGE) --name $(KIND_CLUSTER_NAME)

# Namespace that holds SandboxPolicy objects in the e2e. deploy/controller/10-rbac.yaml
# carries the per-namespace bundle Role for `sandboxes`, so it must exist first.
E2E_POLICY_NAMESPACE ?= sandboxes

kind-deploy: kind-load kind-openbao ## Install CRDs, profiles and the controller from deploy/ into kind, signing with OpenBao
	@echo "Applying CRDs..."
	@$(KIND_KUBECTL) apply -f deploy/crds/
	@$(KIND_KUBECTL) wait --for=condition=Established --timeout=60s -f deploy/crds/
	@$(KIND_KUBECTL) apply -f deploy/controller/00-namespace.yaml
	@$(KIND_KUBECTL) create namespace $(E2E_POLICY_NAMESPACE) --dry-run=client -o yaml | $(KIND_KUBECTL) apply -f -
	@# The deployment signs with OpenBao (ADR-0004) and trusts only its CA, which
	@# must be in place before the pod can start.
	@dev/openbao/use-signer.sh $(KIND_KUBECONFIG) ca
	@echo "Applying controller manifests..."
	@$(KIND_KUBECTL) apply -f deploy/controller/
	@$(KIND_KUBECTL) -n $(NAMESPACE) set image deployment/notaio notaio=$(KIND_IMAGE)
	@$(KIND_KUBECTL) -n $(NAMESPACE) set env deployment/notaio RUST_LOG="$(RUST_LOG)"
	@$(KIND_KUBECTL) -n $(NAMESPACE) rollout status deployment/notaio --timeout=180s
	@echo "Applying built-in profiles..."
	@$(KIND_KUBECTL) apply -f deploy/profiles/

# ----- e2e -------------------------------------------------------------------
#
# The suites run the controller from deploy/ (real image, real RBAC, real
# NetworkPolicy, restricted PSA) against a real API server, and assert on what
# a consumer would see. They prove what unit tests structurally cannot: that the
# apiserver ACCEPTS what notaio builds and that the deployed RBAC is exactly as
# narrow as the threat model says.
#
# Split into independent suites so a red CI job names the contract that broke,
# and so the suites run in parallel on separate clusters:
#
#   kind-e2e-install    CRDs established, controller up, built-in profiles Valid,
#                       examples/ accepted by server-side dry-run
#   kind-e2e-bundle     policy + grant in, signed bundle out, verified with the
#                       public key; version bumps on change and never goes back
#   kind-e2e-rbac       the controller's ServiceAccount can read and write status,
#                       and cannot create, edit or delete policy objects
#   kind-e2e-admission  deploy/admission/gitops-only.yaml refuses direct edits
#                       and still lets the controller write status
#   kind-e2e-openbao    bundles signed by the OpenBao transit key verify against
#                       its public key; without OpenBao nothing is published

# Keep in sync with the matrix in .github/workflows/e2e.yaml. openbao runs last in
# `make kind-e2e`: it switches the signer and restores it afterwards.
E2E_SUITES = install bundle rbac admission openbao

# Every suite verifies bundles against OpenBao's public key, the one setup.sh printed.
E2E_ENV = KUBECONFIG=$(KIND_KUBECONFIG) \
	NOTAIO_E2E_NAMESPACE=$(NAMESPACE) \
	NOTAIO_E2E_POLICY_NAMESPACE=$(E2E_POLICY_NAMESPACE) \
	NOTAIO_E2E_OPENBAO_PUBLIC_KEY="$$(cat $(KIND_OPENBAO_STATE)/notaio-public-key.txt)"

define run-e2e-suite
	@echo "Running $(1) against kind-$(KIND_CLUSTER_NAME)..."
	@$(E2E_ENV) \
	 cargo test -p notaio --test $(1) -- --ignored --nocapture --test-threads=1
	@echo "✓ $(1) passed"
endef

kind-e2e-install: kind-deploy ## e2e: install from deploy/, profiles Valid, examples accepted
	@echo "Validating examples/ against the live schema (server-side dry run)..."
	@$(KIND_KUBECTL) apply --dry-run=server -f examples/
	$(call run-e2e-suite,e2e_install)

kind-e2e-bundle: kind-deploy ## e2e: signed bundle published, verified, versioned
	$(call run-e2e-suite,e2e_bundle)

kind-e2e-rbac: kind-deploy ## e2e: controller RBAC is read plus status writes only
	$(call run-e2e-suite,e2e_rbac)

kind-e2e-admission: kind-deploy ## e2e: GitOps-only admission policy refuses direct edits
	@$(KIND_KUBECTL) apply -f deploy/admission/gitops-only.yaml
	@# Always remove the policy again, pass or fail: it denies every direct edit,
	@# so leaving it behind would break any suite run after this one.
	@$(E2E_ENV) \
	 cargo test -p notaio --test e2e_admission -- --ignored --nocapture --test-threads=1; \
	 rc=$$?; \
	 $(KIND_KUBECTL) delete --ignore-not-found -f deploy/admission/gitops-only.yaml; \
	 exit $$rc
	@echo "✓ e2e_admission passed"

# ----- the OpenBao signer (ADR-0004) -------------------------------------------
#
# The default deployment signs with OpenBao, so every kind suite runs on
# cert-manager and dev/openbao (kind-deploy depends on kind-openbao). The openbao
# suite takes OpenBao away to prove the controller fails closed, so its recipe
# always unseals it again, pass or fail.
CERT_MANAGER_VERSION ?= v1.21.2
CERT_MANAGER_SHA256  ?= e03b668ec8675214af6b0a671699d088f2601fa3878e0dbe1b41d3feafd1879f
KIND_OPENBAO_STATE    = $(CURDIR)/.kind-openbao-$(KIND_CLUSTER_NAME)

kind-cert-manager: kind-create ## Install cert-manager $(CERT_MANAGER_VERSION) into kind (checksum verified)
	@TMP=$$(mktemp -d); \
	curl -sSLf -o $$TMP/cert-manager.yaml https://github.com/cert-manager/cert-manager/releases/download/$(CERT_MANAGER_VERSION)/cert-manager.yaml; \
	if command -v sha256sum >/dev/null 2>&1; then GOT=$$(sha256sum $$TMP/cert-manager.yaml | cut -d' ' -f1); \
	else GOT=$$(shasum -a 256 $$TMP/cert-manager.yaml | cut -d' ' -f1); fi; \
	if [ "$$GOT" != "$(CERT_MANAGER_SHA256)" ]; then echo "ERROR: cert-manager checksum mismatch"; rm -rf $$TMP; exit 1; fi; \
	$(KIND_KUBECTL) apply -f $$TMP/cert-manager.yaml >/dev/null; rm -rf $$TMP
	@$(KIND_KUBECTL) -n cert-manager wait --for=condition=Available deployment --all --timeout=300s >/dev/null
	@# Available is not enough: the webhook refuses requests for a few seconds after
	@# it starts, and setup.sh's first Issuer would fail. Wait until it admits one.
	@for i in $$(seq 1 60); do \
		printf 'apiVersion: cert-manager.io/v1\nkind: Issuer\nmetadata: {name: webhook-probe, namespace: default}\nspec: {selfSigned: {}}\n' \
		| $(KIND_KUBECTL) apply --dry-run=server -f - >/dev/null 2>&1 && break; \
		[ $$i = 60 ] && { echo "ERROR: cert-manager webhook not ready"; exit 1; }; sleep 2; \
	done
	@echo "✓ cert-manager $(CERT_MANAGER_VERSION)"

kind-openbao: kind-cert-manager ## Stand up dev/openbao in kind (state in .kind-openbao-<cluster>)
	@dev/openbao/setup.sh $(KIND_KUBECONFIG) $(KIND_OPENBAO_STATE)

kind-e2e-openbao: kind-deploy ## e2e: bundles signed by the OpenBao transit key; nothing published without it
	@$(E2E_ENV) \
	 cargo test -p notaio --test e2e_openbao -- --ignored --nocapture --test-threads=1; \
	 rc=$$?; \
	 dev/openbao/setup.sh $(KIND_KUBECONFIG) $(KIND_OPENBAO_STATE) >/dev/null || true; \
	 exit $$rc
	@echo "✓ e2e_openbao passed"

# ----- an existing test cluster ------------------------------------------------
#
# The same deployment as kind-deploy, onto a cluster you name, from an image
# already pushed (pin it by digest). Nothing defaults: no cluster or registry
# is ever written into this file.
CLUSTER_KUBECONFIG ?=
NOTAIO_IMAGE       ?=
CLUSTER_KUBECTL     = kubectl --kubeconfig $(CLUSTER_KUBECONFIG)

cluster-deploy: ## Deploy notaio from deploy/ to CLUSTER_KUBECONFIG with NOTAIO_IMAGE (needs dev/openbao there)
	@test -n "$(CLUSTER_KUBECONFIG)" || { echo "set CLUSTER_KUBECONFIG"; exit 1; }
	@test -n "$(NOTAIO_IMAGE)" || { echo "set NOTAIO_IMAGE, pinned by digest"; exit 1; }
	@$(CLUSTER_KUBECTL) apply -f deploy/crds/ >/dev/null
	@$(CLUSTER_KUBECTL) wait --for=condition=Established --timeout=60s -f deploy/crds/ >/dev/null
	@$(CLUSTER_KUBECTL) apply -f deploy/controller/00-namespace.yaml >/dev/null
	@$(CLUSTER_KUBECTL) create namespace $(E2E_POLICY_NAMESPACE) --dry-run=client -o yaml | $(CLUSTER_KUBECTL) apply -f - >/dev/null
	@# Signs with the cluster's dev/openbao (ADR-0004): its CA first, or the pod cannot start.
	@dev/openbao/use-signer.sh $(CLUSTER_KUBECONFIG) ca
	@$(CLUSTER_KUBECTL) apply -f deploy/controller/ >/dev/null
	@$(CLUSTER_KUBECTL) -n $(NAMESPACE) set image deployment/notaio notaio=$(NOTAIO_IMAGE) >/dev/null
	@$(CLUSTER_KUBECTL) -n $(NAMESPACE) rollout status deployment/notaio --timeout=180s >/dev/null
	@$(CLUSTER_KUBECTL) apply -f deploy/profiles/ >/dev/null
	@echo "✓ notaio deployed ($(NOTAIO_IMAGE))"

cluster-undeploy: ## Remove everything cluster-deploy created from CLUSTER_KUBECONFIG
	@test -n "$(CLUSTER_KUBECONFIG)" || { echo "set CLUSTER_KUBECONFIG"; exit 1; }
	-@$(CLUSTER_KUBECTL) delete --ignore-not-found -f deploy/admission/gitops-only.yaml >/dev/null 2>&1
	-@$(CLUSTER_KUBECTL) delete --ignore-not-found -f deploy/profiles/ >/dev/null 2>&1
	-@$(CLUSTER_KUBECTL) delete --ignore-not-found -f deploy/crds/ >/dev/null 2>&1
	-@$(CLUSTER_KUBECTL) delete --ignore-not-found -f deploy/controller/ --wait=false >/dev/null 2>&1
	-@$(CLUSTER_KUBECTL) delete namespace $(E2E_POLICY_NAMESPACE) $(NAMESPACE) --ignore-not-found --wait=true --timeout=120s >/dev/null 2>&1
	@echo "✓ notaio removed"

openbao-e2e: ## e2e_openbao on an existing test cluster with dev/openbao (CLUSTER_KUBECONFIG, OPENBAO_STATE, NOTAIO_IMAGE)
	@test -n "$(OPENBAO_STATE)" || { echo "set OPENBAO_STATE to the directory setup.sh wrote"; exit 1; }
	@$(MAKE) -s cluster-deploy
	@KUBECONFIG=$(CLUSTER_KUBECONFIG) \
	 NOTAIO_E2E_NAMESPACE=$(NAMESPACE) \
	 NOTAIO_E2E_POLICY_NAMESPACE=$(E2E_POLICY_NAMESPACE) \
	 NOTAIO_E2E_OPENBAO_PUBLIC_KEY="$$(cat $(OPENBAO_STATE)/notaio-public-key.txt)" \
	 cargo test -p notaio --test e2e_openbao -- --ignored --nocapture --test-threads=1; \
	 rc=$$?; \
	 dev/openbao/setup.sh $(CLUSTER_KUBECONFIG) $(OPENBAO_STATE) >/dev/null || true; \
	 $(MAKE) -s cluster-undeploy; \
	 exit $$rc

kind-e2e: ## Run every e2e suite, in sequence, against one kind cluster
	@# From the recipe, not as prerequisites: the suites share one cluster and
	@# must not be parallelised by `make -j`.
	@set -e; for suite in $(E2E_SUITES); do $(MAKE) kind-e2e-$$suite; done
	@echo ""
	@echo "✓ all e2e suites passed ($(E2E_SUITES))"

kind-e2e-ci: ## Run e2e suite $(E2E_SUITE) in CI; dumps diagnostics on failure, always deletes the cluster
	@set -e; \
	case "$(E2E_SUITE)" in \
	  all) target=kind-e2e ;; \
	  "") echo "E2E_SUITE is required (one of: $(E2E_SUITES), all)"; exit 1 ;; \
	  *) target=kind-e2e-$(E2E_SUITE) ;; \
	esac; \
	if $(MAKE) $$target; then \
	  rc=0; \
	else \
	  rc=$$?; \
	  echo "::error::e2e suite '$(E2E_SUITE)' failed, dumping cluster state"; \
	  $(MAKE) kind-e2e-logs || true; \
	fi; \
	$(MAKE) kind-delete || true; \
	rm -f $(KIND_KUBECONFIG); \
	exit $$rc

kind-e2e-logs: ## Dump controller and policy state (run this when an e2e fails)
	@echo "── controller deployment ──"
	-@$(KIND_KUBECTL) -n $(NAMESPACE) describe deployment/notaio
	@echo "── controller logs ──"
	-@$(KIND_KUBECTL) -n $(NAMESPACE) logs deployment/notaio --tail=200
	@echo "── policy objects ──"
	-@$(KIND_KUBECTL) get sandboxprofiles,sandboxpolicies,knobgrants,exposureceilings -A -o yaml
	@echo "── bundles ──"
	-@$(KIND_KUBECTL) get configmaps -A -l app.kubernetes.io/managed-by=notaio -o yaml
	@echo "── events ──"
	-@$(KIND_KUBECTL) get events -A --sort-by=.lastTimestamp | tail -50
	@echo "── openbao (if installed) ──"
	-@$(KIND_KUBECTL) -n openbao get pods -o wide 2>/dev/null
	-@$(KIND_KUBECTL) -n openbao logs deploy/openbao --tail=200 2>/dev/null | grep -v '"type"' | tail -50
