#![allow(clippy::unwrap_used, clippy::expect_used)]

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

fn run(
    policy: &SandboxPolicy,
    profile: Option<&SandboxProfile>,
    grants: &[KnobGrant],
    prev: Option<(u64, &str)>,
) -> EvalOutput {
    let ceilings = Ceilings::default();
    evaluate(&EvalInput {
        now: now(),
        policy,
        profile,
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
        host: "git.internal.example".into(),
        port: 443,
        methods: vec![],
        paths: vec![],
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
