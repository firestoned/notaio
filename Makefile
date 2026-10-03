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
        kind-e2e-install kind-e2e-bundle kind-e2e-rbac kind-e2e-admission \
        docs docs-serve docs-clean docs-linkcheck

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
manifests: ## Regenerate deploy/crds and deploy/profiles from the Rust types
	cargo run -q -p notaio --bin crdgen -- crds > deploy/crds/sandbox.firestoned.io.yaml
	cargo run -q -p notaio --bin crdgen -- profiles > deploy/profiles/builtin.yaml

manifests-check: ## Fail if deploy/crds or deploy/profiles drifted from the Rust types
	@cargo run -q -p notaio --bin crdgen -- crds | diff -u deploy/crds/sandbox.firestoned.io.yaml -
	@cargo run -q -p notaio --bin crdgen -- profiles | diff -u deploy/profiles/builtin.yaml -

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
	@.github/tools/linkinator/node_modules/.bin/linkinator docs/site/ --recurse --verbosity error

docs-clean: ## Remove docs/site/ and docs/.venv/
	@rm -rf docs/site/ docs/.venv/
	@echo "✓ Documentation artefacts cleaned"

# ----- CI gating -------------------------------------------------------------
#
# The build workflow has no workflow-level `paths:` filter: a workflow that never
# triggers reports no status at all, so a required check would stay pending
# forever. The path filter lives here instead and the `changes` job applies it
# per job, so a docs-only PR skips the expensive jobs and still reports.
CI_CODE_PATHS_RE ?= ^(crates/|deploy/|docs/air/|scripts/|Cargo\.toml$$|Cargo\.lock$$|deny\.toml$$|clippy\.toml$$|Makefile$$|Dockerfile|\.github/workflows/build\.yaml$$)

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

kind-deploy: kind-load ## Install CRDs, profiles and the controller from deploy/ into kind
	@echo "Applying CRDs..."
	@$(KIND_KUBECTL) apply -f deploy/crds/
	@$(KIND_KUBECTL) wait --for=condition=Established --timeout=60s -f deploy/crds/
	@$(KIND_KUBECTL) apply -f deploy/controller/00-namespace.yaml
	@$(KIND_KUBECTL) create namespace $(E2E_POLICY_NAMESPACE) --dry-run=client -o yaml | $(KIND_KUBECTL) apply -f -
	@# Development signer only (ADR-0002, open decision 2). Created once and never
	@# rotated here: the e2e verifies bundles against the seed it reads back.
	@if ! $(KIND_KUBECTL) -n $(NAMESPACE) get secret notaio-dev-signing-key >/dev/null 2>&1; then \
		echo "Creating a throwaway development signing key..."; \
		$(KIND_KUBECTL) -n $(NAMESPACE) create secret generic notaio-dev-signing-key \
			--from-literal=seed="$$(head -c 32 /dev/urandom | base64)"; \
	fi
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

# Keep in sync with the matrix in .github/workflows/e2e.yaml.
E2E_SUITES = install bundle rbac admission

define run-e2e-suite
	@echo "Running $(1) against kind-$(KIND_CLUSTER_NAME)..."
	@KUBECONFIG=$(KIND_KUBECONFIG) \
	 NOTAIO_E2E_NAMESPACE=$(NAMESPACE) \
	 NOTAIO_E2E_POLICY_NAMESPACE=$(E2E_POLICY_NAMESPACE) \
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
	@KUBECONFIG=$(KIND_KUBECONFIG) \
	 NOTAIO_E2E_NAMESPACE=$(NAMESPACE) \
	 NOTAIO_E2E_POLICY_NAMESPACE=$(E2E_POLICY_NAMESPACE) \
	 cargo test -p notaio --test e2e_admission -- --ignored --nocapture --test-threads=1; \
	 rc=$$?; \
	 $(KIND_KUBECTL) delete --ignore-not-found -f deploy/admission/gitops-only.yaml; \
	 exit $$rc
	@echo "✓ e2e_admission passed"

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
	-@$(KIND_KUBECTL) get sandboxprofiles,sandboxpolicies,knobgrants -A -o yaml
	@echo "── bundles ──"
	-@$(KIND_KUBECTL) get configmaps -A -l app.kubernetes.io/managed-by=notaio -o yaml
	@echo "── events ──"
	-@$(KIND_KUBECTL) get events -A --sort-by=.lastTimestamp | tail -50
