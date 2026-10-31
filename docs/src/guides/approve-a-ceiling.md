# Approve an exposure ceiling

For the `Security` role. An [ExposureCeiling](../concepts/ceilings.md) is your record that a
policy may reach exactly this much, and no more. Lint rules check that a policy fits its knobs;
only you can say that a team should reach these systems.

## What you are approving

The policy's computed maximum exposure, grants included: what a fully compromised sandbox under
it could reach. notaio computes it and reports it even when nothing is published:

```sh
kubectl -n team-docs get sandboxpolicy docs-agent -o jsonpath='{.status.maxExposure}' | jq
```

```json
{
  "audiences": [{ "name": "internal-git", "access": "Read", "ttlSeconds": 300,
                  "endpoints": ["git.foo.io:443"] }],
  "egressHosts": [],
  "writeCapable": false,
  "tools": [],
  "leaseMaxSeconds": 14400
}
```

On a pull request, `notaio lint` names each part of it the ceiling does not cover, one `L014`
finding per audience, host, tool or lease value, and `L015` when there is no ceiling at all.

## Questions to ask

- Does each audience match what this team's work needs? Is `Write` justified, and is
  `writeCapable` what you expect? Is each audience's token used only at the endpoints you expect?
- Is every egress host one you would let a hostile agent reach with this team's tokens?
- Are the tools pinned by digest, and do you trust their manifests?
- Is the lease as short as the work allows?
- Do the active grants (`status.activeGrants`) explain any difference from the profile?

## Recording it

Write the ceiling to cover exactly the exposure you accept, and add your approval with a reference
to the pull request:

```yaml
  approvals:
    - role: Security
      reference: https://github.com/<org>/<policy-repo>/pull/<n>
```

A ceiling wider than the exposure is allowed, but it pre-approves future widening without
another review. Prefer an exact one.

## Keeping it a second approval

Put ceilings under their own path in the policy repository and give that path to the security
approvers in CODEOWNERS. Then widening a policy always takes two reviews: the team's change, and
yours. The controller cannot edit ceilings, and the GitOps-only admission policy refuses direct
edits to them.
