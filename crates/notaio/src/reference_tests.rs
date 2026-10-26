use std::collections::BTreeSet;

use super::*;

fn page_named(name: &str) -> String {
    pages()
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| c)
        .unwrap()
}

#[test]
fn every_page_says_it_is_generated_and_is_deterministic() {
    let a = pages();
    assert_eq!(a, pages(), "two renders differ, so the CI check would flap");
    for (name, content) in a {
        assert!(content.starts_with(GENERATED), "{name}");
        assert!(
            content.ends_with('\n') && !content.ends_with("\n\n"),
            "{name}"
        );
    }
}

#[test]
fn every_lint_code_is_documented() {
    let p = page_named("lint-rules.md");
    for (code, _) in sandboxpolicy_core::lint::RULES
        .iter()
        .chain(crate::lint::INPUT_RULES)
    {
        assert!(p.contains(&format!("| `{code}` |")), "{code} missing");
    }
}

#[test]
fn every_knob_is_in_the_register_and_the_profile_matrix() {
    let k = page_named("knobs.md");
    let p = page_named("profiles.md");
    for knob in REGISTRY {
        assert!(
            k.contains(&format!("| `{}` |", knob.id)),
            "{} not in knobs.md",
            knob.id
        );
        assert!(
            p.contains(&format!("| `{}` |", knob.id)),
            "{} not in profiles.md",
            knob.id
        );
    }
    assert!(k.contains("(Lab only)"));
    // Fortress is R0 everywhere: its column never shows anything looser.
    assert!(
        p.contains("| `K8.4` | Break-glass access | R0 | R0 | R0 | R0 |"),
        "{p}"
    );
}

#[test]
fn every_kind_and_its_fields_are_documented() {
    let p = page_named("crds.md");
    for heading in [
        "## SandboxProfile",
        "## SandboxPolicy",
        "## KnobGrant",
        "## ExposureCeiling",
    ] {
        assert!(p.contains(heading), "{heading}");
    }
    for field in [
        "| `profileRef` | string | yes |",
        "| `audiences[].access` | string: `Read`, `Write` |",
        "| `knobs` | map of string: `R0`, `R1`, `R2`, `R3` |",
        "| `expiresAt` | string | yes |",
        "| `maxLeaseSeconds` | integer | yes |",
        "| `maxExposure.writeCapable` |",
    ] {
        assert!(p.contains(field), "{field} missing from:\n{p}");
    }
}

#[test]
fn every_metric_is_documented_with_its_labels() {
    let p = page_named("metrics.md");
    for row in [
        "| `notaio_relaxations_past_expiry` | gauge |  |",
        "| `notaio_fail_open_total` | counter | `check` |",
        "| `notaio_relaxation_age_seconds` | gauge | `grant`, `namespace` |",
        "| `notaio_bundle_age_seconds` | gauge | `namespace`, `policy` |",
        "| `notaio_policies` | gauge | `reason` |",
    ] {
        assert!(p.contains(row), "{row} missing from:\n{p}");
    }
    // Every family the renderer serves has a row.
    assert_eq!(metric_rows().len(), 12, "{p}");
}

#[test]
fn the_cli_page_shows_every_subcommand() {
    let p = page_named("cli.md");
    assert!(p.contains("## notaio controller"));
    assert!(p.contains("## notaio lint"));
    assert!(p.contains("--format"));
    assert!(p.contains("--now"));
}

/// Every `NOTAIO_*` name in the sources that read the environment.
fn names_in_sources() -> BTreeSet<String> {
    let sources = [
        include_str!("main.rs"),
        include_str!("cli.rs"),
        include_str!("openbao.rs"),
        include_str!("http.rs"),
        include_str!("signer.rs"),
        include_str!("reconcile.rs"),
    ];
    let mut out = BTreeSet::new();
    for src in sources {
        let mut rest = src;
        while let Some(i) = rest.find("NOTAIO_") {
            let tail = &rest[i..];
            let end = tail
                .find(|c: char| !(c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()))
                .unwrap_or(tail.len());
            out.insert(tail[..end].trim_end_matches('_').to_string());
            rest = &tail[end..];
        }
    }
    out
}

#[test]
fn every_variable_read_is_documented() {
    let documented: BTreeSet<String> = environment_variables()
        .into_iter()
        .map(|(n, _, _)| n.to_string())
        .collect();
    let read = names_in_sources();
    let missing: Vec<_> = read.difference(&documented).collect();
    assert!(missing.is_empty(), "read but not documented: {missing:?}");
    let stale: Vec<_> = documented.difference(&read).collect();
    assert!(stale.is_empty(), "documented but never read: {stale:?}");
}
