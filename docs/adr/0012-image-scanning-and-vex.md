# ADR-0012: Image scanning and VEX

- **Status:** Accepted
- **Date:** 2026-10-29
- **Deciders:** Erick Bourgeois
- **Related:** ADR-0009 (release images and their supply chain), whose follow-up this is; banlieue
  ADR-0006 (Grype, OpenVEX), whose curated VEX this follows; the threat-model mapping's gap "known
  vulnerabilities in notaio's image"

## Context

Since the binary is built with `cargo auditable`, notaio's image SBOM lists every Rust crate, so a
scanner can see what the image ships. Grype on the image finds nothing in those crates. It finds
31 matches, all in Debian packages of the distroless base (glibc, the GCC runtime libraries,
zlib), and none has a fix upstream: each is "won't fix" or "not fixed".

Most of them cannot affect notaio, for reasons that are facts about its binary: it does not link
zlib or libstdc++, it does not import the glibc functions an advisory names, it has no RPATH. A
scan that reports them anyway trains its readers to ignore it. A scan that hides them on someone's
say-so is worse.

## Decision

1. **Grype scans every published image**, on `main` and on releases, pinned to a version. Its
   findings go to GitHub Code Scanning as SARIF. The build fails on a finding of High or Critical
   severity that has a fix.
2. **OpenVEX statements in `.vex/`**, one file per CVE, say why notaio is not affected. They are
   merged with `vexctl` into one document, which Grype applies, which is attested to the image
   digest with Cosign (`--type openvex`), and which each release attaches as `notaio.vex.json`.
3. **A statement is only made on mechanical evidence about the binary**, recorded in
   `.vex/.evidence.json` and of four kinds: a library it does not link, symbols it does not import,
   no RPATH or RUNPATH (with the Deployment's non-root, no-escalation settings), and the
   architectures it is built for. A finding without such evidence stays reported, however
   unlikely it is to matter.
4. **`make vex-evidence` checks the evidence against the release binary on every pull request**,
   for both architectures. A statement fails the build the moment its fact stops being true: a
   new dependency that links zlib, or a crate that starts calling `glob`, re-opens the finding
   instead of leaving it hidden. Every statement needs evidence, and all evidence a statement.
5. **No automatic derivation yet.** banlieue derives VEX automatically for two image variants;
   notaio has one image and about twenty statements, so they are written by hand and checked by
   machine. That can change if the count grows.

## Consequences

- Of the 31 matches today, 25 are suppressed by 19 statements, and 6 stay reported: the resolver's
  search-list assertion (notaio does use the resolver), nscd, and four old glibc hardening
  bypasses. None blocks a build, because none has a fix.
- A base image update that fixes a package shows as a disappeared finding; one that introduces a
  fixable High fails the build until the base is bumped (Dependabot proposes the digest).
- A consumer or auditor gets the VEX document three ways: attested to the image, attached to the
  release, and in `.vex/` in the repository, each statement with the fact behind it.
- Two pinned tools join the build: Grype and `vexctl`, both downloaded by version and checked
  against their published checksums.
- **CALM step deferred:** notaio has no CALM model yet. `docs/architecture.md` records the change.
