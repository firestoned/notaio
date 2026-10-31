#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Duration, TimeZone, Utc};
use sandboxpolicy_api::*;
use sandboxpolicy_core::ceilings::Ceilings;
use sandboxpolicy_core::{builtin, evaluate, registry, EvalInput, EvalOutput};
use sandboxpolicy_types::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap()
}

fn profile(name: &str) -> SandboxProfile {
    let mut p = SandboxProfile::new(name, builtin::profile(name).unwrap());
    p.metadata.generation = Some(1);
    p
}

fn audience(name: &str, access: Access) -> AudienceGrant {
    AudienceGrant {
        name: name.into(),
        access,
        scopes: vec!["repo:read".into()],
        ttl_seconds: None,
        endpoints: vec![format!("{name}.internal.example:443")],
        rules: vec![],
    }
}

fn policy(profile_ref: &str, audiences: Vec<AudienceGrant>) -> SandboxPolicy {
    let mut p = SandboxPolicy::new(
        "agent-default",
        SandboxPolicySpec {
            profile_ref: profile_ref.into(),
            subjects: Subjects {
                groups: vec!["algo-devs".into()],
            },
            audiences,
            egress: vec![],
            tools: vec![],
            lease: None,
            budgets: None,
        },
    );
    p.metadata.namespace = Some("sandboxes".into());
    p.metadata.uid = Some("uid-1".into());
    p.metadata.generation = Some(1);
    p
}

fn grant(name: &str, knob: &str, step: Step, days: i64, roles: &[Role]) -> KnobGrant {
    KnobGrant::new(
        name,
        KnobGrantSpec {
            policy_ref: "agent-default".into(),
            knob: KnobId::new(knob),
            step,
            owner: "erick".into(),
            reason: "needed for the build toolchain".into(),
            expires_at: now() + Duration::days(days),
            approvals: roles
                .iter()
                .map(|r| Approval {
                    role: *r,
                    reference: "https://example.invalid/pr/1".into(),
                })
                .collect(),
        },
    )
}

/// A ceiling that covers anything `policy` names (ADR-0007), so tests about something else are not
/// refused for want of one.
fn covering_ceiling(policy: &SandboxPolicy) -> ExposureCeiling {
    let spec = &policy.spec;
    let mut c = ExposureCeiling::new(
        policy.metadata.name.as_deref().unwrap(),
        ExposureCeilingSpec {
            audiences: spec
                .audiences
                .iter()
                .map(|a| CeilingAudience {
                    name: a.name.clone(),
                    access: Access::Write,
                    max_ttl_seconds: u32::MAX,
                    endpoints: a.endpoints.clone(),
                })
                .collect(),
            egress_hosts: spec.egress.iter().map(|e| e.endpoint.clone()).collect(),
            tools: spec.tools.iter().map(|t| t.name.clone()).collect(),
            max_lease_seconds: u32::MAX,
            approvals: vec![Approval {
                role: Role::Security,
                reference: "https://example.invalid/pr/2".into(),
            }],
        },
    );
    c.metadata.namespace = policy.metadata.namespace.clone();
    c
}

fn run(
    policy: &SandboxPolicy,
    profile: Option<&SandboxProfile>,
    grants: &[KnobGrant],
    prev: Option<(u64, &str)>,
) -> EvalOutput {
    let ceiling = covering_ceiling(policy);
    run_with_ceiling(policy, profile, grants, prev, Some(&ceiling))
}

fn run_with_ceiling(
    policy: &SandboxPolicy,
    profile: Option<&SandboxProfile>,
    grants: &[KnobGrant],
    prev: Option<(u64, &str)>,
    exposure_ceiling: Option<&ExposureCeiling>,
) -> EvalOutput {
    let ceilings = Ceilings::default();
    evaluate(&EvalInput {
        now: now(),
        policy,
        profile,
        exposure_ceiling,
        grants,
        prev_version: prev.map(|p| p.0),
        prev_digest: prev.map(|p| p.1),
        prev_conditions: &[],
        bundle_lifetime: Duration::hours(1),
        ceilings: &ceilings,
    })
}

fn ready(o: &EvalOutput) -> bool {
    o.status
        .conditions
        .iter()
        .any(|c| c.type_ == "Ready" && c.status == "True")
}

#[test]
fn registry_matches_profiles() {
    // Every knob in the registry is set by every profile, no profile names an unknown knob, and no
    // profile step exceeds the knob's loosest step.
    for (name, p) in builtin::all() {
        for def in registry::REGISTRY {
            let step = p.knobs.get(&KnobId::new(def.id));
            assert!(step.is_some(), "{name} does not set {}", def.id);
            assert!(*step.unwrap() <= def.max, "{name} {} above max", def.id);
        }
        assert_eq!(
            p.knobs.len(),
            registry::REGISTRY.len(),
            "{name} has extra knobs"
        );
    }
}

#[test]
fn hardened_read_only_policy_compiles() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let o = run(&po, Some(&pr), &[], None);
    assert!(ready(&o), "{:?}", o.status.conditions);
    let b = o.bundle.unwrap();
    assert_eq!(b.meta.version, 1);
    assert_eq!(b.content.audiences[0].ttl_seconds, Some(300)); // K5.1 R0 ceiling
    assert_eq!(b.content.knobs.len(), registry::REGISTRY.len());
    assert_eq!(b.meta.not_after, now() + Duration::hours(1));
    let exp = o.status.max_exposure.unwrap();
    assert!(!exp.write_capable);
    assert_eq!(exp.audiences.len(), 1);
}

#[test]
fn version_is_stable_until_content_changes() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let first = run(&po, Some(&pr), &[], None);
    let digest = first.status.bundle_digest.clone().unwrap();

    let again = run(&po, Some(&pr), &[], Some((1, &digest)));
    assert_eq!(again.status.bundle_version, Some(1));

    let mut changed = policy("hardened", vec![audience("internal-wiki", Access::Read)]);
    changed.metadata.generation = Some(2);
    let bumped = run(&changed, Some(&pr), &[], Some((1, &digest)));
    assert_eq!(bumped.status.bundle_version, Some(2));
}

#[test]
fn missing_profile_fails_closed() {
    let po = policy("nope", vec![]);
    let o = run(&po, None, &[], None);
    assert!(o.bundle.is_none() && !ready(&o));
    assert_eq!(o.status.conditions[0].reason, "ProfileNotFound");
}

#[test]
fn fortress_refuses_audiences_and_egress() {
    let pr = profile("fortress");
    let mut po = policy("fortress", vec![audience("internal-git", Access::Read)]);
    po.spec.egress.push(EgressRule {
        endpoint: "git.internal.example:443".into(),
        rules: vec![],
    });
    let o = run(&po, Some(&pr), &[], None);
    assert!(o.bundle.is_none());
    let codes: Vec<_> = o.findings.iter().map(|f| f.code).collect();
    assert!(
        codes.contains(&"L005") && codes.contains(&"L007"),
        "{codes:?}"
    );
}

#[test]
fn write_access_needs_k52_and_ttl_is_capped() {
    let pr = profile("hardened");
    let mut a = audience("internal-git", Access::Write);
    a.ttl_seconds = Some(3600);
    let o = run(&policy("hardened", vec![a]), Some(&pr), &[], None);
    let n = o.findings.iter().filter(|f| f.code == "L006").count();
    assert_eq!(n, 2, "{:?}", o.findings);
}

#[test]
fn wildcard_audience_is_never_allowed() {
    let pr = profile("standard");
    let o = run(
        &policy("standard", vec![audience("*", Access::Read)]),
        Some(&pr),
        &[],
        None,
    );
    assert!(o.findings.iter().any(|f| f.code == "L005"));
}

#[test]
fn unpinned_tool_is_refused() {
    let pr = profile("hardened");
    let mut po = policy("hardened", vec![]);
    po.spec.tools.push(ToolRef {
        name: "lint".into(),
        manifest_digest: "latest".into(),
    });
    let o = run(&po, Some(&pr), &[], None);
    assert!(o.findings.iter().any(|f| f.code == "L008"));
}

#[test]
fn valid_grant_applies_and_clamps_bundle_validity() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    // K2.3 package installation: Hardened is R0, one step to R1, Platform owner approves.
    let g = grant("pkg", "K2.3", Step::R1, 10, &[Role::PlatformOwner]);
    let o = run(&po, Some(&pr), &[g], None);
    assert!(ready(&o));
    assert_eq!(o.grants[0].phase, GrantPhase::Active);
    assert_eq!(o.status.active_grants, vec!["pkg".to_string()]);
    let b = o.bundle.unwrap();
    assert_eq!(b.content.knobs[&KnobId::new("K2.3")], Step::R1);
    // The grant outlives the one hour bundle lifetime here, so validity stays at one hour.
    assert_eq!(b.meta.not_after, now() + Duration::hours(1));

    // A grant that expires in 30 minutes clamps the bundle to that instant.
    let mut short = grant("pkg", "K2.3", Step::R1, 0, &[Role::PlatformOwner]);
    short.spec.expires_at = now() + Duration::minutes(30);
    let o = run(&po, Some(&pr), &[short], None);
    assert_eq!(
        o.bundle.unwrap().meta.not_after,
        now() + Duration::minutes(30)
    );
}

#[test]
fn grant_that_jumps_two_steps_is_rejected() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    let g = grant("jump", "K2.3", Step::R2, 10, &[Role::PlatformOwner]);
    let o = run(&po, Some(&pr), &[g], None);
    assert_eq!(o.grants[0].phase, GrantPhase::Rejected);
    assert!(o.grants[0].message.contains("L011"));
    // A rejected grant changes nothing.
    assert_eq!(
        o.bundle.unwrap().content.knobs[&KnobId::new("K2.3")],
        Step::R0
    );
}

#[test]
fn grant_without_required_approvals_is_rejected() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    // K5.2 needs both the data owner and security.
    let g = grant("write", "K5.2", Step::R1, 10, &[Role::DataOwner]);
    let o = run(&po, Some(&pr), &[g], None);
    assert_eq!(o.grants[0].phase, GrantPhase::Rejected);
    assert!(o.grants[0].message.contains("L013"));
}

#[test]
fn grant_lifetime_is_capped_per_step() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    let g = grant("long", "K2.3", Step::R1, 91, &[Role::PlatformOwner]);
    let o = run(&po, Some(&pr), &[g], None);
    assert!(o.grants[0].message.contains("L012"));
}

#[test]
fn expired_grant_is_reported_and_not_applied() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    let mut g = grant("old", "K2.3", Step::R1, 10, &[Role::PlatformOwner]);
    g.spec.expires_at = now() - Duration::minutes(1);
    let o = run(&po, Some(&pr), &[g], None);
    assert_eq!(o.grants[0].phase, GrantPhase::Expired);
    assert!(o.status.active_grants.is_empty());
}

#[test]
fn grant_that_completes_the_triangle_is_rejected() {
    // Standard already has sensitive data (K2.2 R1) and an outbound write scope (K5.2 R1).
    // Raising K1.1 from R1 to R2 adds untrusted content, which completes the triangle.
    let pr = profile("standard");
    let po = policy("standard", vec![]);
    let g = grant("web", "K1.1", Step::R2, 10, &[Role::Security]);
    let o = run(&po, Some(&pr), &[g], None);
    assert_eq!(o.grants[0].phase, GrantPhase::Rejected);
    assert!(
        o.grants[0].message.contains("L003"),
        "{}",
        o.grants[0].message
    );
    assert_eq!(
        o.bundle.unwrap().content.knobs[&KnobId::new("K1.1")],
        Step::R1
    );
}

#[test]
fn grants_for_other_policies_are_ignored() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    let mut g = grant("pkg", "K2.3", Step::R1, 10, &[Role::PlatformOwner]);
    g.spec.policy_ref = "someone-else".into();
    let o = run(&po, Some(&pr), &[g], None);
    assert!(o.grants.is_empty());
}

#[test]
fn second_grant_on_the_same_knob_is_rejected() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![]);
    let a = grant("a-first", "K2.3", Step::R1, 10, &[Role::PlatformOwner]);
    let b = grant("b-second", "K2.3", Step::R1, 10, &[Role::PlatformOwner]);
    let o = run(&po, Some(&pr), &[b, a], None);
    let phases: Vec<_> = o
        .grants
        .iter()
        .map(|g| (g.name.as_str(), g.phase))
        .collect();
    assert_eq!(
        phases,
        vec![
            ("a-first", GrantPhase::Active),
            ("b-second", GrantPhase::Rejected)
        ]
    );
}

#[test]
fn invalid_profile_blocks_everything() {
    let mut pr = profile("standard");
    pr.spec.knobs.insert(KnobId::new("K1.1"), Step::R2);
    pr.spec.knobs.insert(KnobId::new("K4.1"), Step::R2); // completes the triangle on Internal
    let o = run(&policy("standard", vec![]), Some(&pr), &[], None);
    assert!(o.bundle.is_none());
    assert_eq!(o.status.conditions[0].reason, "InvalidProfile");
}

#[test]
fn compiled_lists_are_sorted_so_the_digest_is_stable() {
    let pr = profile("standard");
    let a = policy(
        "standard",
        vec![audience("b", Access::Read), audience("a", Access::Read)],
    );
    let b = policy(
        "standard",
        vec![audience("a", Access::Read), audience("b", Access::Read)],
    );
    let da = run(&a, Some(&pr), &[], None).status.bundle_digest;
    let db = run(&b, Some(&pr), &[], None).status.bundle_digest;
    assert_eq!(da, db);
}

#[test]
fn content_without_a_canonical_form_fails_closed() {
    // ADR-0005, decision 2. The default K4.3 ceilings keep maxEgressBytes far below 2^53, so lift
    // them to reach the bound: lint L009 then passes and only the canonical form can refuse.
    let pr = profile("hardened");
    let mut po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    po.spec.budgets = Some(Budgets {
        max_tool_calls: 500,
        max_wall_clock_seconds: 3600,
        max_egress_bytes: 1 << 53,
    });
    let ceilings = Ceilings {
        egress_bytes: [u64::MAX; 3],
        ..Ceilings::default()
    };
    let ceiling = covering_ceiling(&po);
    let o = evaluate(&EvalInput {
        now: now(),
        policy: &po,
        profile: Some(&pr),
        exposure_ceiling: Some(&ceiling),
        grants: &[],
        prev_version: None,
        prev_digest: None,
        prev_conditions: &[],
        bundle_lifetime: Duration::hours(1),
        ceilings: &ceilings,
    });
    assert!(o.bundle.is_none(), "a bundle was compiled");
    assert!(!ready(&o));
    assert!(
        o.status
            .conditions
            .iter()
            .any(|c| c.type_ == "Ready" && c.reason == "NotCanonical"),
        "{:?}",
        o.status.conditions
    );
    assert_eq!(o.status.bundle_digest, None);
}

#[test]
fn rejected_grant_keeps_its_structured_findings() {
    // ADR-0006: notaio lint reports each finding with its own code, so a rejection must not be
    // flattened into a string.
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let g = grant("jump", "K2.3", Step::R2, 7, &[Role::PlatformOwner]);
    let o = run(&po, Some(&pr), &[g], None);
    let r = &o.grants[0];
    assert_eq!(r.phase, GrantPhase::Rejected);
    assert!(
        r.findings.iter().any(|f| f.code == "L011"),
        "{:?}",
        r.findings
    );
}

#[test]
fn rules_table_matches_the_documented_codes() {
    // The data table and the rustdoc table in lint.rs describe the same rules.
    let doc = include_str!("../src/lint.rs");
    for (code, meaning) in sandboxpolicy_core::lint::RULES {
        assert!(
            doc.contains(&format!(
                "//! | {code} | {}",
                meaning.replace("LabOnly", "`LabOnly`")
            )),
            "{code} differs between RULES and the documented table"
        );
    }
    assert_eq!(sandboxpolicy_core::lint::RULES.len(), 18);
}

fn ready_reason(o: &EvalOutput) -> Option<&str> {
    o.status
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")
        .map(|c| c.reason.as_str())
}

fn has_code(o: &EvalOutput, code: &str) -> bool {
    o.findings.iter().any(|f| f.code == code)
}

fn read_ceiling(po: &SandboxPolicy, ttl: u32, lease: u32) -> ExposureCeiling {
    let mut c = covering_ceiling(po);
    for a in &mut c.spec.audiences {
        a.access = Access::Read;
        a.max_ttl_seconds = ttl;
    }
    c.spec.max_lease_seconds = lease;
    c
}

#[test]
fn l015_no_ceiling_fails_closed_and_still_reports_exposure() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let o = run_with_ceiling(&po, Some(&pr), &[], None, None);
    assert!(o.bundle.is_none());
    assert!(!ready(&o));
    assert_eq!(ready_reason(&o), Some("NoApprovedCeiling"));
    assert!(has_code(&o, "L015"), "{:?}", o.findings);
    // Approvers need to see what they are being asked to approve.
    assert!(o.status.max_exposure.is_some());
}

#[test]
fn l014_exposure_not_covered_fails_closed() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    // Hardened gives a 300 second token and a 4 hour lease. Each ceiling below misses one.
    for ceiling in [
        read_ceiling(&po, 60, u32::MAX),
        read_ceiling(&po, 300, 3600),
    ] {
        let o = run_with_ceiling(&po, Some(&pr), &[], None, Some(&ceiling));
        assert!(o.bundle.is_none(), "published past {:?}", ceiling.spec);
        assert_eq!(ready_reason(&o), Some("ExposureAboveCeiling"));
        assert!(has_code(&o, "L014"), "{:?}", o.findings);
    }
    // Exactly at the ceiling is covered.
    let exact = read_ceiling(&po, 300, 4 * 3600);
    let o = run_with_ceiling(&po, Some(&pr), &[], None, Some(&exact));
    assert!(ready(&o), "{:?}", o.findings);
}

#[test]
fn l014_an_audience_missing_from_the_ceiling_is_not_covered() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let mut c = covering_ceiling(&po);
    c.spec.audiences[0].name = "something-else".into();
    let o = run_with_ceiling(&po, Some(&pr), &[], None, Some(&c));
    assert!(has_code(&o, "L014"), "{:?}", o.findings);
    assert!(o.bundle.is_none());
}

#[test]
fn l014_a_grant_that_widens_exposure_past_the_ceiling_is_refused() {
    // K5.1 R1 raises the default token lifetime from 300 to 900 seconds. The grant itself is
    // valid; the exposure it produces is not approved.
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let g = grant("ttl", "K5.1", Step::R1, 7, &[Role::Security]);
    let c = read_ceiling(&po, 300, u32::MAX);
    let o = run_with_ceiling(&po, Some(&pr), &[g], None, Some(&c));
    assert_eq!(o.grants[0].phase, GrantPhase::Active);
    assert!(has_code(&o, "L014"), "{:?}", o.findings);
    assert!(o.bundle.is_none());
}

#[test]
fn l016_ceiling_without_a_security_approval_fails_closed() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let mut c = covering_ceiling(&po);
    c.spec.approvals[0].role = Role::PlatformOwner;
    let o = run_with_ceiling(&po, Some(&pr), &[], None, Some(&c));
    assert!(has_code(&o, "L016"), "{:?}", o.findings);
    assert!(o.bundle.is_none());
}

// ---- ADR-0013: routes per audience, direct egress as endpoint and rules ----------------------

fn rule(method: &str, path: &str) -> EndpointRule {
    EndpointRule {
        method: method.into(),
        path: path.into(),
    }
}

fn codes_of(o: &EvalOutput) -> Vec<&'static str> {
    o.findings.iter().map(|f| f.code).collect()
}

#[test]
fn audience_endpoints_are_brokered_so_hardened_allows_them() {
    // Hardened is K4.1 R0: no direct egress, but a brokered audience endpoint with rules is fine.
    let pr = profile("hardened");
    let mut a = audience("internal-git", Access::Read);
    a.rules = vec![rule("GET", "/repos/**"), rule("*", "/health")];
    let po = policy("hardened", vec![a]);
    let o = run(&po, Some(&pr), &[], None);
    assert!(ready(&o), "{:?}", o.findings);
    let b = o.bundle.unwrap();
    assert_eq!(
        b.content.audiences[0].endpoints,
        vec!["internal-git.internal.example:443"]
    );
    // Rules are sorted in the canonical form, so their order in the policy does not matter.
    assert_eq!(
        b.content.audiences[0].rules,
        vec![rule("*", "/health"), rule("GET", "/repos/**")]
    );
    assert_eq!(b.schema, "sandbox.firestoned.io/bundle/v3");
}

#[test]
fn l017_refuses_malformed_audience_endpoints_and_rules() {
    let pr = profile("hardened");
    for (endpoints, rules) in [
        (vec!["no-port.example"], vec![]),
        (vec!["git.example:0"], vec![]),
        (vec!["git.example:70000"], vec![]),
        (vec!["*.example:443"], vec![]),
        (vec!["10.0.0.1:443"], vec![]),
        (vec!["[::1]:443"], vec![]),
        (vec!["git.example:443"], vec![rule("GET", "repos")]),
        (vec!["git.example:443"], vec![rule("GET POST", "/")]),
        (vec!["git.example:443"], vec![rule("", "/")]),
    ] {
        let mut a = audience("internal-git", Access::Read);
        a.endpoints = endpoints.iter().map(|e| e.to_string()).collect();
        a.rules = rules;
        let po = policy("hardened", vec![a.clone()]);
        let o = run(&po, Some(&pr), &[], None);
        assert!(o.bundle.is_none(), "{a:?} published");
        assert!(codes_of(&o).contains(&"L017"), "{a:?}: {:?}", o.findings);
    }
}

#[test]
fn l018_refuses_an_endpoint_reachable_two_ways() {
    let pr = profile("standard");
    let a = audience("internal-git", Access::Read);
    let mut b = audience("wiki", Access::Read);
    b.endpoints = a.endpoints.clone();
    let o = run(
        &policy("standard", vec![a.clone(), b]),
        Some(&pr),
        &[],
        None,
    );
    assert!(codes_of(&o).contains(&"L018"), "{:?}", o.findings);

    // Brokered and direct at once.
    let mut po = policy("standard", vec![a.clone()]);
    po.spec.egress.push(EgressRule {
        endpoint: a.endpoints[0].clone(),
        rules: vec![],
    });
    let o = run(&po, Some(&pr), &[], None);
    assert!(codes_of(&o).contains(&"L018"), "{:?}", o.findings);

    // Listing an endpoint twice in one audience is the same way, not two.
    let mut c = audience("internal-git", Access::Read);
    c.endpoints.push(c.endpoints[0].clone());
    let o = run(&policy("standard", vec![c]), Some(&pr), &[], None);
    assert!(!codes_of(&o).contains(&"L018"), "{:?}", o.findings);
}

#[test]
fn l007_checks_direct_egress_in_its_new_shape() {
    // Standard is K4.1 R1: direct egress by name, but no rules (R2) and no wildcard (R3).
    let pr = profile("standard");
    let cases = [
        (
            EgressRule {
                endpoint: "pkg.example:443".into(),
                rules: vec![],
            },
            false,
        ),
        (
            EgressRule {
                endpoint: "pkg.example".into(),
                rules: vec![],
            },
            true,
        ),
        (
            EgressRule {
                endpoint: "192.0.2.1:443".into(),
                rules: vec![],
            },
            true,
        ),
        (
            EgressRule {
                endpoint: "*.example:443".into(),
                rules: vec![],
            },
            true,
        ),
        (
            EgressRule {
                endpoint: "pkg.example:443".into(),
                rules: vec![rule("GET", "/simple/**")],
            },
            true,
        ),
    ];
    for (e, refused) in cases {
        let mut po = policy("standard", vec![]);
        po.spec.egress.push(e.clone());
        let o = run(&po, Some(&pr), &[], None);
        assert_eq!(
            codes_of(&o).contains(&"L007"),
            refused,
            "{e:?}: {:?}",
            o.findings
        );
    }
}

#[test]
fn l014_refuses_an_audience_endpoint_the_ceiling_does_not_list() {
    let pr = profile("hardened");
    let po = policy("hardened", vec![audience("internal-git", Access::Read)]);
    let mut ceiling = covering_ceiling(&po);
    ceiling.spec.audiences[0].endpoints = vec!["other.internal.example:443".into()];
    let o = run_with_ceiling(&po, Some(&pr), &[], None, Some(&ceiling));
    assert!(o.bundle.is_none());
    assert!(
        o.findings
            .iter()
            .any(|f| f.code == "L014" && f.path.contains("endpoints")),
        "{:?}",
        o.findings
    );
    // The exposure reports where each audience's token can be used.
    let exposure = o.status.max_exposure.unwrap();
    assert_eq!(
        exposure.audiences[0].endpoints,
        vec!["internal-git.internal.example:443"]
    );
}
