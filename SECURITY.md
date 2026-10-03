# Security Policy

## Supported Versions

notaio is pre-1.0 and under active development. Only the latest commit on `main` receives security
fixes.

| Version | Supported |
| ------- | --------- |
| `main` | ✅ |
| anything older | ❌ |

## Reporting a Vulnerability

**Do not open a public issue for a suspected vulnerability.**

Report it privately via GitHub's
[private vulnerability reporting](https://github.com/firestoned/notaio/security/advisories/new)
("Report a vulnerability" on the repository's Security tab).

Include, where possible:

- the affected component (controller, bundle format or verification, lint rules, manifests,
  admission policy) and the commit;
- steps to reproduce or a proof of concept;
- the impact you believe it has.

You can expect an acknowledgement within **3 business days** and a triage decision (accepted,
needs more information, or not a vulnerability) within **7**. Accepted reports get a fix or
mitigation plan, and credit in the release notes unless you ask otherwise.

## Scope

In scope, with particular interest:

- anything that lets a bundle be accepted by a consumer when it should be refused: a forged or
  unsigned bundle, a downgrade, an expired bundle, a digest or schema mismatch;
- a policy or grant that passes the lint rules while breaking a rule of the threat model (a toxic
  combination, a wildcard audience, a jump of more than one step);
- a way for the controller to create, edit or delete a policy object, read a Secret, or write
  outside the namespaces it is granted;
- a way around `deploy/admission/gitops-only.yaml`.

Out of scope, or known:

- issues that require cluster-admin privileges, which already bypass the admission policy by
  removing it;
- the development signer keeping its key in a Kubernetes Secret. It is a known gap, documented in
  `docs/architecture.md` and ADR-0002 (open decision 2), and is not for production use.

The threat model notaio implements is the AgentSandbox Threat Model and Security Roadmap.
`docs/threat-model-mapping.md` states what this repository enforces and what it does not.
