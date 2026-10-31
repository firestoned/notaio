# Copyright (c) 2026 Erick Bourgeois, notaio
# SPDX-License-Identifier: Apache-2.0
"""Check the rendered release manifests (ADR-0009, amended): the CRDs alone in their own file,
and in the other the image pinned by digest exactly once, no tag reference surviving, every kind
the install needs, and nothing scoped to a namespace the operator has to choose.

    check_release_manifest.py <manifest> <crds manifest> <pinned image reference>
"""

import sys

import yaml

# notaio-<version>.yaml: deploy/controller and deploy/profiles. The admission policy names the
# operator's GitOps identity, and the bundle Role is per policy namespace (deploy/policy-namespace),
# so neither is in it.
EXPECTED = {
    "Namespace": 1,
    "ServiceAccount": 1,
    "ClusterRole": 1,
    "ClusterRoleBinding": 1,
    "Role": 1,
    "RoleBinding": 1,
    "Deployment": 1,
    "PodDisruptionBudget": 1,
    "NetworkPolicy": 1,
    "SandboxProfile": 4,
}
FORBIDDEN = {
    "CustomResourceDefinition",
    "ValidatingAdmissionPolicy",
    "ValidatingAdmissionPolicyBinding",
}
# notaio-<version>-crds.yaml: the four CRDs and nothing else.
EXPECTED_CRDS = {"CustomResourceDefinition": 4}
CONTROLLER_NAMESPACE = "notaio-system"


def fail(msg: str) -> None:
    print(f"check_release_manifest: {msg}", file=sys.stderr)
    sys.exit(1)


def load(path: str) -> tuple[str, list[dict], dict[str, int]]:
    text = open(path, encoding="utf-8").read()
    docs = [d for d in yaml.safe_load_all(text) if d]
    kinds: dict[str, int] = {}
    for d in docs:
        kinds[d["kind"]] = kinds.get(d["kind"], 0) + 1
    return text, docs, kinds


def main() -> None:
    path, crds_path, pinned = sys.argv[1], sys.argv[2], sys.argv[3]

    _, _, crd_kinds = load(crds_path)
    if crd_kinds != EXPECTED_CRDS:
        fail(f"the CRDs manifest must hold exactly {EXPECTED_CRDS}, found {crd_kinds}")

    text, docs, kinds = load(path)
    for kind, n in EXPECTED.items():
        if kinds.get(kind, 0) != n:
            fail(f"expected {n} {kind}, found {kinds.get(kind, 0)}")
    for kind in FORBIDDEN & kinds.keys():
        fail(f"{kind} must not be in the install manifest")
    for d in docs:
        ns = d.get("metadata", {}).get("namespace")
        if d["kind"] in ("Role", "RoleBinding") and ns != CONTROLLER_NAMESPACE:
            fail(f"{d['kind']} {d['metadata']['name']} is scoped to {ns!r}, a namespace the operator chooses")
    images = [
        c["image"]
        for d in docs
        if d["kind"] == "Deployment"
        for c in d["spec"]["template"]["spec"]["containers"]
    ]
    if images != [pinned]:
        fail(f"expected the image pinned to {pinned}, found {images}")
    if text.count("ghcr.io/firestoned/notaio") != 1:
        fail("the image is named more than once, or a tag reference survived")


if __name__ == "__main__":
    main()
