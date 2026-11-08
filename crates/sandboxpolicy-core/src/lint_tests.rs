// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::builtin;

fn knobs(pairs: &[(&str, Step)]) -> BTreeMap<KnobId, Step> {
    pairs.iter().map(|(k, s)| (KnobId::new(k), *s)).collect()
}

fn codes(f: &[Finding]) -> Vec<&'static str> {
    f.iter().map(|f| f.code).collect()
}

#[test]
fn builtin_profiles_are_clean() {
    for (name, p) in builtin::all() {
        let f = lint_knobs(p.data_scope, &p.knobs);
        assert!(!has_errors(&f), "{name}: {f:?}");
    }
}

#[test]
fn triangle_fires_for_internal_profile() {
    let k = knobs(&[("K2.2", Step::R1), ("K1.1", Step::R2), ("K4.1", Step::R2)]);
    assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
}

#[test]
fn any_two_legs_are_allowed() {
    let sens_out = knobs(&[("K2.2", Step::R1), ("K4.1", Step::R2)]);
    let sens_untrusted = knobs(&[("K2.2", Step::R1), ("K1.1", Step::R2)]);
    let untrusted_out = knobs(&[("K1.1", Step::R2), ("K4.1", Step::R2)]);
    for k in [sens_out, sens_untrusted, untrusted_out] {
        assert!(!codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
    }
}

#[test]
fn write_scope_counts_as_an_outbound_channel() {
    let k = knobs(&[("K5.3", Step::R1), ("K1.1", Step::R2), ("K5.2", Step::R1)]);
    assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
}

#[test]
fn lab_only_scope_clears_the_sensitive_leg() {
    let k = knobs(&[("K2.2", Step::R2), ("K1.1", Step::R3), ("K4.1", Step::R3)]);
    assert!(!codes(&lint_knobs(DataScope::LabOnly, &k)).contains(&"L003"));
    assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L003"));
}

#[test]
fn lab_only_knobs_are_refused_elsewhere() {
    let k = knobs(&[("K6.3", Step::R1)]);
    assert!(codes(&lint_knobs(DataScope::Internal, &k)).contains(&"L004"));
    assert!(!has_errors(&lint_knobs(DataScope::LabOnly, &k)));
}

#[test]
fn unknown_and_over_max_knobs() {
    let k = knobs(&[("K9.9", Step::R1), ("K4.4", Step::R3)]);
    let c = codes(&lint_knobs(DataScope::Internal, &k));
    assert!(c.contains(&"L001") && c.contains(&"L002"));
}

fn ceiling_with(audiences: &[(&str, Option<DataScope>)]) -> ExposureCeilingSpec {
    ExposureCeilingSpec {
        audiences: audiences
            .iter()
            .map(|(name, scope)| sandboxpolicy_api::CeilingAudience {
                name: (*name).to_string(),
                access: Access::Read,
                max_ttl_seconds: 300,
                endpoints: vec![],
                data_scope: *scope,
            })
            .collect(),
        egress_hosts: vec![],
        tools: vec![],
        max_lease_seconds: 3600,
        approvals: vec![],
    }
}

#[test]
fn l021_two_ceilings_classifying_one_audience_differently_warn_on_both() {
    // ADR-0017, Consequences: two ceilings can disagree; notaio lint, which sees both, says so.
    let lab = ceiling_with(&[("proof.example.com", Some(DataScope::LabOnly))]);
    let internal = ceiling_with(&[("proof.example.com", Some(DataScope::Internal))]);
    let f = lint_classifications(&[("a/lab", &lab), ("b/internal", &internal)]);
    assert_eq!(f.len(), 2, "{f:?}");
    for (i, finding) in &f {
        assert_eq!(finding.code, "L021");
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.path, "spec.audiences[0].dataScope");
        let other = if *i == 0 { "b/internal" } else { "a/lab" };
        assert!(finding.message.contains(other), "{}", finding.message);
    }
    let mut on: Vec<usize> = f.iter().map(|(i, _)| *i).collect();
    on.sort_unstable();
    assert_eq!(on, vec![0, 1]);
}

#[test]
fn l021_unset_counts_as_internal() {
    // ADR-0017, decision 2: no dataScope is Internal, so unset against LabOnly disagrees, and
    // unset against Internal does not.
    let lab = ceiling_with(&[("x.example.com", Some(DataScope::LabOnly))]);
    let unset = ceiling_with(&[("x.example.com", None)]);
    let internal = ceiling_with(&[("x.example.com", Some(DataScope::Internal))]);
    assert_eq!(lint_classifications(&[("a", &lab), ("b", &unset)]).len(), 2);
    assert!(lint_classifications(&[("b", &unset), ("c", &internal)]).is_empty());
}

#[test]
fn l021_agreeing_or_distinct_audiences_are_clean() {
    let a = ceiling_with(&[
        ("same.example.com", Some(DataScope::LabOnly)),
        ("only-a.example.com", Some(DataScope::LabOnly)),
    ]);
    let b = ceiling_with(&[
        ("same.example.com", Some(DataScope::LabOnly)),
        ("only-b.example.com", None),
    ]);
    assert!(lint_classifications(&[("a", &a), ("b", &b)]).is_empty());
    assert!(lint_classifications(&[("a", &a)]).is_empty());
    assert!(lint_classifications(&[]).is_empty());
}

#[test]
fn l021_points_at_the_audience_index_in_each_ceiling() {
    let a = ceiling_with(&[
        ("other.example.com", None),
        ("x.example.com", Some(DataScope::LabOnly)),
    ]);
    let b = ceiling_with(&[("x.example.com", None)]);
    let f = lint_classifications(&[("a", &a), ("b", &b)]);
    let path_of = |i: usize| {
        f.iter()
            .find(|(c, _)| *c == i)
            .map(|(_, f)| f.path.clone())
            .unwrap()
    };
    assert_eq!(path_of(0), "spec.audiences[1].dataScope");
    assert_eq!(path_of(1), "spec.audiences[0].dataScope");
}

#[test]
fn l021_is_in_the_rule_table() {
    assert!(RULES.iter().any(|(c, _)| *c == "L021"));
}
