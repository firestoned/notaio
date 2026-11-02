# ADR-0015: Policies delivered as signed OCI artifacts

- **Status:** Accepted
- **Date:** 2026-11-02
- **Deciders:** Erick Bourgeois
- **Related:** roadmap 03; ADR-0007 (the ExposureCeiling and Security's approval); ADR-0002 (the
  bundle and its signature); ADR-0009 (how notaio's own releases are signed); threat model T8.2

Implemented outside this repository, in a policy repository and its cluster: a CI job that renders
the repository into two OCI artifacts and signs them, a script for Security's signature, and two
Flux `OCIRepository` objects that verify them. The [guide](../guides/deliver-signed-policies.md)
has all three. Proven on a test cluster: a tightening reached a new bundle version through it, and
a ceiling signed only by CI, and a policy pushed with no signature at all, were refused with the
reason on the source and the cluster left as it was.

## Context

A policy repository delivered by GitOps (roadmap 01) puts on the cluster whatever its default
branch holds. Review shows on pull requests, but whether it can block a merge depends on the hosting
plan: on GitHub's free plan, branch protection is not available for private repositories, and
anyone who can push can change what is applied. notaio's own checks still hold (a policy above its
approved ceiling is never signed, ADR-0007), but the ceiling, the line Security approves, arrives by
the same path as everything else. Its approval is a field in the object, which anyone who can push
can write.

notaio signs bundles, and that signature cannot move earlier. A bundle is computed in the cluster
from the policy, its profile, the grants active at that moment and its ceiling, re-signed as grants
expire and as its lifetime runs out, and verified by every consumer, which trusts no Kubernetes API
it reads it from. A signature over what someone wrote is checked once, when it is pulled, and says
nothing about what notaio derived from it. So the question is not who signs the bundle but who
signs what reaches the cluster.

## Decision

1. **The policy repository is delivered as two OCI artifacts,** each pulled by its own Flux
   `OCIRepository` that accepts only its own signer, and applied by its own Kustomization:
   `policies` (the namespaces, notaio's bundle Role, the SandboxPolicies) and `ceilings` (the
   ExposureCeilings). CI renders each namespace with kustomize and splits it by kind, so the
   repository's layout does not change.
2. **CI signs both, keyless, from the default branch, after `notaio lint`.** For `policies` that is
   the signature Flux requires, matched on the workflow's OIDC issuer and subject. It proves "built
   from the default branch after lint", not "reviewed"; that is enough because a policy can never
   exceed its ceiling.
3. **Security signs `ceilings` by hand, with a key only Security holds, and Flux requires that
   signature.** CI's signature on `ceilings` is provenance: Security's script refuses a digest CI
   did not sign, shows the content and the commit, and signs by digest. An `OCIRepository`
   verifies one method, a key or a keyless identity, so Flux enforces Security's and the script
   enforces CI's. A push credential, or a compromised workflow, can therefore produce a valid
   `policies` artifact but never a valid ceiling.
4. **Security's signature has no transparency log entry.** A keyless signature names its signer in
   a public log: for a person, their email beside the digest of an artifact from a private
   repository. A key, signed with a Cosign signing config that names no service, publishes
   nothing. CI's keyless signatures are in the public log; they name the repository and its
   workflow, not the content.
5. **Signatures are over digests; Flux follows a tag and verifies the digest it resolves to.** A
   tag moved to an unsigned or wrongly signed digest is refused and the last verified content stays
   applied. `ceilings` is pushed only when its content changes, so Security signs only when there
   is something new to approve.
6. **GitOps-only admission stays** (`deploy/admission/gitops-only.yaml`): it is what stops an edit
   made after Flux applied. notaio's bundle signature stays, unchanged.
7. **Git delivery stays the documented simple path.** Signed delivery is the recommended one when
   the policy repository's hosting cannot enforce review, or when the ceilings need a signer of
   their own.

## Consequences

- A ceiling reaches the cluster only with Security's signature, whoever can push. This closes the
  hosting-plan review gap where it matters; for policies it is bounded by the ceilings, as before.
- **Accepted risk:** a `policies` artifact signed by CI proves a build, not a review. Anyone who can
  push to the default branch, or who compromises the workflow, can ship a policy, though never one
  above its Security-signed ceiling. *Revisit when* push access is wider than Security, or a policy
  can do harm inside its ceiling that review should have caught: then Security signs `policies`
  too, or the hosting enforces review (a protected environment with required reviewers can then
  gate CI's own signature).
- Security keeps a signing key: lost, nothing new ships until a new key is trusted; leaked, whoever
  holds it can sign ceilings. Its public key is in the policy repository and, as a Secret, in the
  cluster; a new key is a reviewed change to both.
- The registry joins the delivery path. If it is down, Flux keeps the last applied artifacts and
  nothing new ships. Flux pulls with a read-only token, which replaces a git deploy key only if
  nothing else is delivered from git.
- Two Cosign 3 details the procedure depends on: Cosign reads a `COSIGN_KEY` environment variable
  as `--key` on every command, which breaks a keyless check run in the same shell; and signing
  with no transparency log uses a signing config that names no service, the replacement for the
  deprecated `--tlog-upload=false`. Verifying such a signature always warns that the log was
  skipped, which here is the design.
- **CALM step deferred:** notaio has no CALM model yet.
