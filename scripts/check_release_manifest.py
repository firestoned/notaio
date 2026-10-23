"""Check a rendered release manifest (ADR-0009): the image is pinned by digest exactly once, no tag
reference survives, and every kind the install needs is there and parses.

    check_release_manifest.py <manifest> <pinned image reference>
"""

import sys

import yaml

# What an install from the release manifest must contain (deploy/crds, deploy/controller,
# deploy/profiles). The admission policy is deliberately not in it (ADR-0009, decision 5).
EXPECTED = {
    "CustomResourceDefinition": 4,
    "Namespace": 1,
    "ServiceAccount": 1,
    "ClusterRole": 1,
    "ClusterRoleBinding": 1,
    "Role": 1,
    "RoleBinding": 1,
    "Deployment": 1,
    "NetworkPolicy": 1,
    "SandboxProfile": 4,
}
FORBIDDEN = {"ValidatingAdmissionPolicy", "ValidatingAdmissionPolicyBinding"}


def fail(msg: str) -> None:
    print(f"check_release_manifest: {msg}", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    path, pinned = sys.argv[1], sys.argv[2]
    text = open(path, encoding="utf-8").read()
    docs = [d for d in yaml.safe_load_all(text) if d]
    kinds: dict[str, int] = {}
    for d in docs:
        kinds[d["kind"]] = kinds.get(d["kind"], 0) + 1
    for kind, n in EXPECTED.items():
        if kinds.get(kind, 0) != n:
            fail(f"expected {n} {kind}, found {kinds.get(kind, 0)}")
    for kind in FORBIDDEN & kinds.keys():
        fail(f"{kind} must not be in the install manifest")
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
