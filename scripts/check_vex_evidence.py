# Copyright (c) 2026 Erick Bourgeois, notaio
# SPDX-License-Identifier: Apache-2.0
"""Check that every VEX statement's evidence still holds for the release binary (ADR-0012).

Each `.vex/<CVE>.json` says notaio is not affected, for a reason that is a fact about the binary.
`.vex/.evidence.json` records those facts; this checks them against the binary CI just built, so a
statement fails the build the moment it stops being true, instead of silently hiding a finding.

    check_vex_evidence.py <binary> [<vex dir>] [<deployment>] [<dockerfile>]

Evidence kinds:
    libraries_not_linked  shared libraries that must not be in the binary's DT_NEEDED
    symbols_not_imported  dynamic symbols the binary must not import
    no_rpath              the binary has no DT_RPATH or DT_RUNPATH (and runs non-root, not setuid)
    architectures         the machines the binary may be built for
"""

import json
import pathlib
import re
import subprocess
import sys

KINDS = {"libraries_not_linked", "symbols_not_imported", "no_rpath", "architectures"}


def parse_needed(dynamic: str) -> set[str]:
    """DT_NEEDED entries from `readelf -d`."""
    return set(re.findall(r"\(NEEDED\)\s+Shared library: \[([^\]]+)\]", dynamic))


def parse_has_rpath(dynamic: str) -> bool:
    return bool(re.search(r"\((RPATH|RUNPATH)\)", dynamic))


def parse_imports(nm_out: str) -> set[str]:
    """Undefined dynamic symbols from `nm -D --undefined-only`, without version suffixes."""
    out = set()
    for line in nm_out.splitlines():
        parts = line.split()
        if parts:
            out.add(parts[-1].split("@")[0])
    return out


def parse_machine(header: str) -> str:
    m = re.search(r"Machine:\s+(.*)", header)
    text = m.group(1).strip() if m else ""
    if "X86-64" in text:
        return "x86-64"
    if "AArch64" in text:
        return "aarch64"
    return text


def check(statements: set[str], evidence: dict, facts: dict) -> list[str]:
    """Every problem, empty when the evidence holds. `facts` holds needed, imports, has_rpath,
    machine, and the deployment and dockerfile text."""
    problems = []
    entries = {k: v for k, v in evidence.items() if not k.startswith("_")}
    for cve in sorted(statements - entries.keys()):
        problems.append(f"{cve}: a VEX statement with no evidence entry")
    for cve in sorted(entries.keys() - statements):
        problems.append(f"{cve}: evidence with no VEX statement")
    for cve, ev in sorted(entries.items()):
        if not ev or set(ev) - KINDS:
            problems.append(f"{cve}: evidence must use only {sorted(KINDS)}, got {sorted(ev)}")
        for lib in ev.get("libraries_not_linked", []):
            if lib in facts["needed"]:
                problems.append(f"{cve}: the binary now links {lib}")
        for sym in ev.get("symbols_not_imported", []):
            if sym in facts["imports"]:
                problems.append(f"{cve}: the binary now imports {sym}")
        if ev.get("no_rpath"):
            if facts["has_rpath"]:
                problems.append(f"{cve}: the binary now has DT_RPATH or DT_RUNPATH")
            if "allowPrivilegeEscalation: false" not in facts["deployment"]:
                problems.append(f"{cve}: the Deployment no longer sets allowPrivilegeEscalation: false")
            if "runAsNonRoot: true" not in facts["deployment"]:
                problems.append(f"{cve}: the Deployment no longer sets runAsNonRoot: true")
            if "--chmod=755" not in facts["dockerfile"]:
                problems.append(f"{cve}: the Dockerfile no longer installs the binary mode 0755")
        archs = ev.get("architectures")
        if archs is not None and facts["machine"] not in archs:
            problems.append(f"{cve}: the binary is built for {facts['machine']}, not {archs}")
    return problems


def statements_in(vex_dir: pathlib.Path) -> set[str]:
    out = set()
    for f in sorted(vex_dir.glob("*.json")):
        doc = json.loads(f.read_text(encoding="utf-8"))
        for s in doc.get("statements", []):
            out.add(s["vulnerability"]["name"])
    return out


def run(*cmd: str) -> str:
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    binary = sys.argv[1]
    vex_dir = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else ".vex")
    deployment = pathlib.Path(sys.argv[3] if len(sys.argv) > 3 else "deploy/controller/20-deployment.yaml")
    dockerfile = pathlib.Path(sys.argv[4] if len(sys.argv) > 4 else "Dockerfile")
    dynamic = run("readelf", "-d", binary)
    facts = {
        "needed": parse_needed(dynamic),
        "has_rpath": parse_has_rpath(dynamic),
        "imports": parse_imports(run("nm", "-D", "--undefined-only", binary)),
        "machine": parse_machine(run("readelf", "-h", binary)),
        "deployment": deployment.read_text(encoding="utf-8"),
        "dockerfile": dockerfile.read_text(encoding="utf-8"),
    }
    evidence = json.loads((vex_dir / ".evidence.json").read_text(encoding="utf-8"))
    statements = statements_in(vex_dir)
    problems = check(statements, evidence, facts)
    for p in problems:
        print(f"vex-evidence: {p}", file=sys.stderr)
    if problems:
        return 1
    print(
        f"vex-evidence: {len(statements)} statements hold for {binary} "
        f"({facts['machine']}, {len(facts['imports'])} imported symbols, links {sorted(facts['needed'])})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
