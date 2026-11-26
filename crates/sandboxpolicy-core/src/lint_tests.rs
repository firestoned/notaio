// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::builtin;
use sandboxpolicy_api::Subjects;

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

fn path_rule(path: &str) -> EndpointRule {
    EndpointRule {
        method: "GET".into(),
        path: path.into(),
    }
}

/// A rule path is in the one spelling a consumer admits requests in (mediatore's
/// `path_is_canonical`): a path it would refuse can never match, yet reads as inside a wider rule
/// to a coverage check (L028).
#[test]
fn a_rule_path_no_request_could_have_is_refused() {
    for bad in [
        "/v2/../admin/**",
        "/v2/./x",
        "/v2//x",
        "/v2\\x",
        "/v2/%2e%2e/x",
        "/v2/%2Fx",
        "/v2/%5cx",
        "/v2/%00",
        "/v2/%7f",
        "/v2/%zz",
        "/v2/%2",
        "/v2/\u{7}",
        "v2/x",
    ] {
        assert!(rule_problem(&path_rule(bad)).is_some(), "{bad:?} passed");
    }
}

#[test]
fn ordinary_rule_paths_and_globs_pass() {
    for good in [
        "/",
        "/v1/messages",
        "/repos/**",
        "/firestoned/*/info/refs",
        "/v2/trailing/",
        "/a%20b/%2B",
        "/caf\u{e9}",
        "/.well-known/x",
        "/v2/...",
    ] {
        assert_eq!(rule_problem(&path_rule(good)), None, "{good:?}");
    }
}

// L034 (ADR-0024 decision 3): a group entry that is empty, padded, a pattern, or repeated.

/// A spec on `hardened` with no audiences, egress or tools, so only the subject rules can fire.
fn spec_with_groups(groups: &[&str]) -> SandboxPolicySpec {
    SandboxPolicySpec {
        profile_ref: "hardened".into(),
        subjects: Subjects {
            groups: groups.iter().map(|g| (*g).to_string()).collect(),
        },
        audiences: vec![],
        egress: vec![],
        tools: vec![],
        lease: None,
        budgets: None,
        includes: vec![],
        publish: true,
    }
}

fn subject_findings(groups: &[&str]) -> Vec<Finding> {
    let hardened = builtin::profile("hardened").unwrap();
    lint_policy(
        &hardened.knobs,
        &spec_with_groups(groups),
        &Ceilings::default(),
    )
    .into_iter()
    .filter(|f| f.code == "L034" || f.code == "L010")
    .collect()
}

#[test]
fn l034_well_formed_groups_pass() {
    for groups in [
        &["00000000-0000-0000-0000-000000000000"][..],
        &["algo-devs", "platform:sre", "a b"][..],
    ] {
        assert!(subject_findings(groups).is_empty(), "{groups:?}");
    }
}

#[test]
fn l034_refuses_each_malformed_entry_at_its_index() {
    for (bad, why) in [
        ("", "empty"),
        (" algo-devs", "whitespace"),
        ("algo-devs\t", "whitespace"),
        ("algo-*", "pattern"),
        ("algo-?", "pattern"),
        ("[ab]", "pattern"),
        ("{a,b}", "pattern"),
    ] {
        let f = subject_findings(&["ok", bad]);
        assert_eq!(codes(&f), ["L034"], "{bad:?}");
        assert_eq!(f[0].severity, Severity::Error, "{bad:?}");
        assert_eq!(f[0].path, "subjects.groups[1]", "{bad:?}");
        assert!(f[0].message.contains(why), "{bad:?}: {}", f[0].message);
    }
}

#[test]
fn l034_a_repeated_group_is_refused_on_the_repeat_naming_the_first() {
    let f = subject_findings(&["algo-devs", "sre", "algo-devs"]);
    assert_eq!(codes(&f), ["L034"]);
    assert_eq!(f[0].path, "subjects.groups[2]");
    assert!(
        f[0].message.contains("subjects.groups[0]"),
        "{}",
        f[0].message
    );
}

#[test]
fn l034_matching_is_exact_so_case_and_prefix_variants_are_not_repeats() {
    assert!(subject_findings(&["Algo-Devs", "algo-devs", "corp/algo-devs"]).is_empty());
}

#[test]
fn l034_an_empty_list_is_l010_not_l034() {
    assert_eq!(codes(&subject_findings(&[])), ["L010"]);
}

#[test]
fn l034_is_in_the_rule_table() {
    assert!(RULES.iter().any(|(c, _)| *c == "L034"));
}
