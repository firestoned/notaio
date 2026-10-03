# Built-in Profiles

The four built-in `SandboxProfile` objects, transcribed from the AgentSandbox threat model's
section 10 and generated from `crates/sandboxpolicy-core/src/builtin.rs` by `make manifests`. Every
knob in the register appears in every profile; a test enforces it.

| Profile | Use |
| --- | --- |
| `fortress` | Analysis of sensitive material with no outbound path |
| `hardened` | Code and document work against internal systems, read-only |
| `standard` | Everyday engineering work |
| `lab` | Experiments and tool evaluation on non-production data |

A policy moves a single knob one step above its profile only through a time-boxed, approved
`KnobGrant`.

```yaml title="deploy/profiles/builtin.yaml"
--8<-- "deploy/profiles/builtin.yaml"
```
