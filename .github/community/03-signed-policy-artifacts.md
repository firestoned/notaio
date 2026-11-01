# Roadmap 03: policies delivered as signed OCI artifacts

- **Status:** Not started. The decisions below are recommended, not yet taken. Security owns both
  the policies and the ceilings (decided 2026-11-01).
- **Milestone:** M3 in [`ROADMAPS.md`](../../ROADMAPS.md), hardening what roadmap 01 built
- **Related:** roadmap 01 (the policy repository, Flux, GitOps-only admission, and its risk "review
  is not enforced on GitHub's free plan"); ADR-0007 (the ExposureCeiling and Security's approval);
  ADR-0002 and ADR-0013 (the bundle); ADR-0009 (how notaio's own releases are signed)

## 1. Goal

A ceiling reaches the cluster only if Security signed it, and a policy only if it was built from the
policy repository's `main` after `notaio lint`, each checked by Flux before anything is applied.
Security owns both the policies and the ceilings; what differs is who can produce each signature. Today the policy repository's
`main` is what Flux applies, and on GitHub's free plan anyone with push access can change `main`:
review shows on pull requests but cannot block a merge (roadmap 01, section 6). notaio's in-cluster
checks still hold (a policy above its ceiling is never signed), but the ceiling itself, the line
Security approves, arrives the same way as everything else.

## 2. What this changes, and what it does not

Two signatures, over two different steps:

| Step | Signed by | Checked by | Proves |
| --- | --- | --- | --- |
| Policy repository to cluster (this roadmap) | CI and Security, with cosign | Flux's `OCIRepository`, before applying | these objects are the ones their owners released |
| notaio to consumers (ADR-0002) | notaio, through OpenBao transit | mediatore, the guest, the gateway | this is the effective policy notaio computed, current, at this version or newer |

The bundle signature stays. A bundle is computed in the cluster from the policy, its profile, the
grants active at that moment and its ceiling, and is re-signed at half its lifetime and whenever a
grant expires; consumers read it from the Kubernetes API, which they do not trust. An artifact
signature is checked once, by Flux, when the artifact is pulled, and says nothing about what notaio
derived from it. Roadmap 02 (OpenBao unsealing itself) is unaffected.

GitOps-only admission stays too: it is what stops an edit made after Flux applied.

## 3. Decisions

**D1. Two artifacts, two signers.** Security owns policies and ceilings alike, so the split is not
between teams. It is between two identities: CI's signature, which anyone who can push to `main`
(or who holds the CI's credentials) can cause, and Security's key, which only Security holds.

- *Recommended:* the policy repository publishes `policies` (namespaces, `SandboxPolicy`, notaio's
  bundle Role) and `ceilings` (`ExposureCeiling`) as two OCI artifacts, each pulled by its own
  `OCIRepository` that accepts only its own signer, and applied by its own Kustomization. A
  policy change cannot widen what Security approved, because notaio refuses a policy above its
  ceiling (L014, in the cluster), and the ceiling is only what Security signed.
- *Alternative:* one artifact, signed by hand by Security. The strongest and the simplest to
  reason about, but every policy change, however small, waits for someone to sign it. Worth it if
  push access to the policy repository is ever wider than Security, or if the policies themselves,
  not just the ceilings, need a person's signature.
- *Rejected:* one artifact signed by CI. It would leave the ceilings, the line Security approves,
  signed by whatever can make CI run.

**D2. Who signs, and with what.**

- *Recommended:* `policies` is signed by the policy repository's CI after `notaio lint` passes on
  `main`, keyless (the workflow's identity, matched by Flux's `matchOIDCIdentity` on the
  repository, workflow and `refs/heads/main`). `ceilings` is signed by a Security member, by hand,
  after review, with a key Security alone holds (hardware-backed where possible); Flux checks it
  against that public key.
- Why not keyless for Security as well: Fulcio certificates name the signer, and Rekor publishes
  them. For a person that is their email address in a public log, beside the digest of an artifact
  from a private repository. A key, with no transparency log upload, keeps that private, at the
  cost of managing the key.
- Why CI may sign `policies`: anyone who can push can make CI sign, so this signature proves "built
  from `main` after lint", not "reviewed". That is enough for policies because they are bounded
  by the ceilings; it would not be enough for the ceilings, which is why those are signed by hand.
  Today the people who can push and the people who hold the key are the same; the split still
  means a leaked push credential or a compromised workflow cannot widen a ceiling.
- A paid plan changes this: protected environments with required reviewers would let a workflow
  sign the ceilings only after Security approves the run. On the free plan, environment protection
  is not available for private repositories.

**D3. Where the artifacts live.** The organisation's container registry, as private packages
beside the private policy repository. Flux pulls with a read-only token in a Secret, which replaces
the read-only deploy key it uses today for the git repository.

**D4. Tags and digests.** CI pushes each `main` commit as `<short sha>` and moves `latest`; Flux
follows `latest`. A signature is over a digest, and Flux verifies the digest it resolved, so
moving a tag to an unsigned digest is refused, not followed.

## 4. Work, in order

### Phase A: prove it on kind

Things to confirm, not assume, before writing the guide:

- [ ] Flux (`OCIRepository` with `verify.provider: cosign`) in kind, with a registry in the
      cluster: an artifact signed with a key is applied; an unsigned one, one signed by another
      key, and a tag moved to an unsigned digest are refused, with `OCIRepository` `Ready=False`
      saying why, and nothing of them applied.
- [ ] Which signature formats the Flux version in use verifies: Cosign 3 stores signatures as OCI
      referrers in the new bundle format (ADR-0009). If Flux reads only the older format, sign in
      that format and say so in the guide.
- [ ] A key-signed artifact with no transparency log upload verifies.
- [ ] Two artifacts, two `OCIRepository` objects, two signers: a ceiling signed with the policy
      signer's key is refused.

### Phase B: the policy repository

- [ ] CI on `main`: `notaio lint`, then `flux push artifact` of `policies`, then `cosign sign`
      keyless; the ceilings directory excluded from it.
- [ ] Security's signing, written down as a short procedure and a script: build the `ceilings`
      artifact from a reviewed commit, push it, sign it with Security's key.
- [ ] The commands in the repository's README and `TESTING.md`.

### Phase C: the test cluster

- [ ] The registry token in a Secret; the two `OCIRepository` objects with their `verify` blocks;
      the Kustomizations moved from the git source to them; the git source and its deploy key
      removed once nothing uses them.
- [ ] Repeat roadmap 01's D1 (a tightening reaches a verified bundle) through the new path.
- [ ] Refusals on the real cluster: a `policies` artifact pushed without a signature, and a
      `ceilings` artifact signed by CI, each leave the cluster as it was, with the reason on the
      `OCIRepository`.
- [ ] `make cluster-smoke` after each.

### Phase D: documentation and decision record

- [ ] An ADR in notaio recording the recommended delivery (two artifacts, two signers, Flux
      verification) as the documented path, with git delivery kept as the simple one.
- [ ] A guide: deliver policies as signed OCI artifacts, with the CI workflow, Security's
      procedure, and the `OCIRepository` objects.
- [ ] The threat model: T8.2 (a silent policy edit) gains the signed source as a control before
      admission; the residual risk of CI-signed policies, bounded by Security-signed ceilings.

### Later, separate decision: the source in the bundle

Recording in each bundle which signed artifact (digest and signer) its policy came from would let
a consumer follow the chain from author to enforcement. It changes the bundle's content, so it is
a schema change with its own ADR, and notaio has to learn the source revision without depending
on Flux's API. Not part of this roadmap.

## 5. Done when

On the test cluster, a policy change reaches a verified bundle only through a signed `policies`
artifact, a ceiling only through a `ceilings` artifact Security signed, and each refusal in phase C
leaves the cluster unchanged with the reason on the `OCIRepository`.

## 6. Risks

- **CI-signed policies prove the build, not a review.** Accepted because the ceilings bound them;
  revisit if a policy can ever do harm inside its ceiling that review should have caught, or if
  push access grows beyond Security (then D1's alternative: Security signs both).
- **Security's key is a new secret to keep.** Lost, nothing new ships until a new key is trusted;
  leaked, whoever holds it can sign ceilings. Hardware-backed where possible, and its public key
  in the cluster is a reviewed change.
- **The registry becomes part of the path.** If it is down, Flux keeps the last applied artifacts
  and nothing new ships; what is already applied keeps working.
