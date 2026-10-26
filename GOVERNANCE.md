# Governance

How notaio is run today, and how that changes as the project grows.

## Mission

notaio makes the AgentSandbox threat model's knob register executable. It validates sandbox
policy objects, compiles each policy into a signed, versioned bundle, and publishes it for the
components that enforce it: mediatore, mediatore-guest and the egress gateway. It holds no
hypervisor credentials and no agent traffic reaches it.

## Roles

**Users** run notaio, or write policies for it. They shape it through issues and feedback.

**Contributors** submit changes: code, docs, ADRs, reviews, triage. Anyone can contribute; see
[CONTRIBUTING.md](https://github.com/firestoned/notaio/blob/main/CONTRIBUTING.md).

**Maintainers** are listed in [MAINTAINERS.md](https://github.com/firestoned/notaio/blob/main/MAINTAINERS.md). They:

- review and merge pull requests;
- decide on ADRs and on the roadmap;
- cut releases;
- triage security reports under [SECURITY.md](https://github.com/firestoned/notaio/blob/main/SECURITY.md);
- enforce the [Code of Conduct](https://github.com/firestoned/notaio/blob/main/CODE_OF_CONDUCT.md).

Path-level review ownership is in
[`.github/CODEOWNERS`](https://github.com/firestoned/notaio/blob/main/.github/CODEOWNERS).

## How decisions are made

- **Day to day: lazy consensus.** A pull request that has passed CI and maintainer review merges
  unless a maintainer objects. Once there are two or more maintainers, a change needs approval
  from a maintainer other than its author.
- **Architecture: ADRs.** An architecturally significant change is proposed as an ADR in
  [`docs/adr/`](https://github.com/firestoned/notaio/tree/main/docs/adr) and is accepted when the
  maintainers agree. ADRs are never deleted or renumbered; a reversed decision is marked
  superseded and links forward.
- **Security posture: the threat model.** The AgentSandbox threat model is the source of truth for
  the knob register and the profiles. No lint rule, ceiling or knob maximum is loosened without a
  change to the threat model to point to, and where this code and the model disagree, the
  disagreement is raised, never quietly resolved in either direction.
- **When consensus fails: a vote.** Any maintainer may call a vote on the issue or pull request. It
  passes with a simple majority of all maintainers, open for at least five business days. With a
  single maintainer, that maintainer decides and records the reasoning in the ADR or pull request.
- **Changes to this document** need a two-thirds majority of maintainers.

## Becoming a maintainer

A contributor becomes a maintainer when they have shown, over several months:

- sustained, high-quality contributions (code, reviews, docs or ADRs);
- sound judgement in review, especially on security, since notaio signs what every sandbox
  enforces;
- that they work by the project's method (ADR first, tests first) and its Code of Conduct.

Any maintainer may nominate a contributor. The nomination passes with a two-thirds majority of the
existing maintainers, and the new maintainer is added to MAINTAINERS.md and `.github/CODEOWNERS`
in the same pull request.

A maintainer can step down at any time and becomes an emeritus maintainer. A maintainer inactive
for six months may be moved to emeritus by a two-thirds majority of the others, after being asked.

## Releases and support

Releases are cut from `main` by a maintainer, tagged `vX.Y.Z`
([Semantic Versioning](https://semver.org/)), and published by CI with a keyless signature,
SBOMs, build provenance and an install manifest pinned by digest
([ADR-0009](https://github.com/firestoned/notaio/blob/main/docs/adr/0009-release-and-supply-chain.md)).
Each release's notes are in [CHANGELOG.md](https://github.com/firestoned/notaio/blob/main/CHANGELOG.md).

Before 1.0, only the latest release and `main` are supported (see
[SECURITY.md](https://github.com/firestoned/notaio/blob/main/SECURITY.md)). A change to the CRDs or
to the bundle schema is called out in the release notes and in an ADR.

## Code of Conduct

Everyone taking part follows the
[Code of Conduct](https://github.com/firestoned/notaio/blob/main/CODE_OF_CONDUCT.md). Maintainers
are responsible for enforcing it.
