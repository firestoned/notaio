# Guides

Task-oriented walkthroughs. For what the objects are, see [Concepts](../concepts/index.md).

| Guide | For | You will |
| --- | --- | --- |
| [Install notaio](install.md) | Platform operator | Install a release, pinned by digest, with its RBAC, NetworkPolicy and admission policy |
| [Set up the OpenBao signer](openbao-signer.md) | Platform and security | Create the transit key, the sign-only policy and the Kubernetes auth role, and hand the public key to consumers |
| [Write a policy](write-a-policy.md) | Team owning sandboxes | Pick a profile, write a SandboxPolicy, check it, and get it Ready |
| [Grant a relaxation](grant-a-relaxation.md) | Team, with its approvers | Relax one knob by one step for a bounded time |
| [Approve an exposure ceiling](approve-a-ceiling.md) | Security | Review what a policy can reach and record the approval |
| [Lint a policy repository](lint-in-ci.md) | Policy repository owners | Run `notaio lint` on every pull request, with inline findings |
| [Consume a bundle](consume-a-bundle.md) | mediatore, mediatore-guest, gateway authors | Verify bundles and keep a version floor |
| [Observe notaio](observability.md) | Operators | Scrape the metrics, alert on the must-be-zero series, read the Events |
| [Rotate the signing key](rotate-the-signing-key.md) | Platform and security | Move to a new key version without a gap at any consumer |
| [Troubleshooting](troubleshooting.md) | Anyone | Find out why a policy is not Ready, by its reason |
