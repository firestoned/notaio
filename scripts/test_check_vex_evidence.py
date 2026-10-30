"""Tests for check_vex_evidence.py: each kind of evidence fails when the fact behind it changes."""

import unittest

import check_vex_evidence as c

DYNAMIC = """
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
 0x000000000000000c (INIT)               0x1000
"""
NM = """                 U getaddrinfo@GLIBC_2.2.5
                 U _Unwind_GetIP@GCC_3.0
                 w __gmon_start__
"""
HEADER = "  Machine:                           Advanced Micro Devices X86-64\n"
DEPLOYMENT = "runAsNonRoot: true\nallowPrivilegeEscalation: false\n"
DOCKERFILE = "COPY --chmod=755 binaries/x/notaio /usr/local/bin/notaio\n"


def facts(**over):
    f = {
        "needed": c.parse_needed(DYNAMIC),
        "has_rpath": c.parse_has_rpath(DYNAMIC),
        "imports": c.parse_imports(NM),
        "machine": c.parse_machine(HEADER),
        "deployment": DEPLOYMENT,
        "dockerfile": DOCKERFILE,
    }
    f.update(over)
    return f


EVIDENCE = {
    "_comment": "ignored",
    "CVE-A": {"libraries_not_linked": ["libz.so.1"]},
    "CVE-B": {"symbols_not_imported": ["glob", "regexec"]},
    "CVE-C": {"no_rpath": True},
    "CVE-D": {"architectures": ["x86-64", "aarch64"]},
}
STATEMENTS = {"CVE-A", "CVE-B", "CVE-C", "CVE-D"}


class Parsing(unittest.TestCase):
    def test_reads_the_binary_facts(self):
        self.assertEqual(c.parse_needed(DYNAMIC), {"libgcc_s.so.1", "libc.so.6"})
        self.assertEqual(c.parse_imports(NM), {"getaddrinfo", "_Unwind_GetIP", "__gmon_start__"})
        self.assertFalse(c.parse_has_rpath(DYNAMIC))
        self.assertTrue(c.parse_has_rpath(DYNAMIC + " 0x1d (RUNPATH) Library runpath: [$ORIGIN]\n"))
        self.assertEqual(c.parse_machine(HEADER), "x86-64")
        self.assertEqual(c.parse_machine("  Machine: AArch64\n"), "aarch64")


class Check(unittest.TestCase):
    def test_holding_evidence_passes(self):
        self.assertEqual(c.check(STATEMENTS, EVIDENCE, facts()), [])

    def test_a_newly_linked_library_fails(self):
        p = c.check(STATEMENTS, EVIDENCE, facts(needed={"libc.so.6", "libz.so.1"}))
        self.assertEqual(p, ["CVE-A: the binary now links libz.so.1"])

    def test_a_newly_imported_symbol_fails(self):
        p = c.check(STATEMENTS, EVIDENCE, facts(imports={"regexec"}))
        self.assertEqual(p, ["CVE-B: the binary now imports regexec"])

    def test_an_rpath_or_a_weaker_deployment_fails(self):
        self.assertTrue(c.check(STATEMENTS, EVIDENCE, facts(has_rpath=True)))
        self.assertTrue(c.check(STATEMENTS, EVIDENCE, facts(deployment="runAsNonRoot: true\n")))
        self.assertTrue(c.check(STATEMENTS, EVIDENCE, facts(dockerfile="COPY notaio /\n")))

    def test_another_architecture_fails(self):
        p = c.check(STATEMENTS, EVIDENCE, facts(machine="PowerPC64"))
        self.assertEqual(len(p), 1)
        self.assertIn("PowerPC64", p[0])

    def test_statements_and_evidence_must_pair_up(self):
        p = c.check(STATEMENTS | {"CVE-E"}, EVIDENCE, facts())
        self.assertEqual(p, ["CVE-E: a VEX statement with no evidence entry"])
        p = c.check(STATEMENTS - {"CVE-D"}, EVIDENCE, facts())
        self.assertEqual(p, ["CVE-D: evidence with no VEX statement"])

    def test_unknown_or_empty_evidence_fails(self):
        bad = dict(EVIDENCE, **{"CVE-D": {"trust_me": True}})
        self.assertTrue(c.check(STATEMENTS, bad, facts()))
        empty = dict(EVIDENCE, **{"CVE-D": {}})
        self.assertTrue(c.check(STATEMENTS, empty, facts()))


if __name__ == "__main__":
    unittest.main()
