# Architecture Decision Records

notaio is built ADR first (Architecture Driven Development): a decision is recorded here before the
code that implements it, and every implemented ADR ends with a full pass over the
[Threat Model Mapping](../security/threat-model-mapping.md).

| ADR | Title |
| --- | --- |
| [ADR-0001](0001-separate-project.md) | notaio is a separate project |
| [ADR-0002](0002-bundle-contract.md) | The bundle contract |
| [ADR-0003](0003-bundle-distribution.md) | Bundle distribution through a ConfigMap, relayed to guests by mediatore |
| [ADR-0004](0004-signing-key-custody.md) | The production signing key lives in OpenBao transit |
| [ADR-0005](0005-canonical-form-rfc-8785.md) | The bundle content is canonicalised with RFC 8785 |
| [ADR-0006](0006-notaio-lint-cli.md) | `notaio lint`, the policy check for pull requests |
| [ADR-0007](0007-exposure-ceiling.md) | ExposureCeiling, the approved maximum exposure of a policy |
| [ADR-0008](0008-consumer-version-floor.md) | The consumer's persisted version floor |
| [ADR-0009](0009-release-and-supply-chain.md) | Release images and their supply chain |
| [ADR-0010](0010-health-metrics-and-events.md) | Health, metrics and Events |
| [ADR-0011](0011-leader-election.md) | Leader election and replicas |

ADRs live in [`docs/adr/`](https://github.com/firestoned/notaio/tree/main/docs/adr) in the
repository and are never renumbered.
