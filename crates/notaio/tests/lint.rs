#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

//! `notaio lint`, the engine (ADR-0006). Every input code I001 to I008 has a test here that fails
//! when its check is removed.

use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use notaio::lint::{lint_sources, lint_sources_with, Level, Report, Source};
use sandboxpolicy_core::ceilings::Ceilings;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 0, 0, 0).unwrap()
}

fn src(name: &str, text: &str) -> Source {
    Source {
        path: PathBuf::from(name),
        text: text.to_string(),
    }
}

fn policy(name: &str, profile: &str, access: &str) -> String {
    format!(
        r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxPolicy
metadata:
  name: {name}
  namespace: sandboxes
spec:
  profileRef: {profile}
  subjects:
    groups: ["00000000-0000-0000-0000-000000000000"]
  audiences:
    - name: internal-git
      access: {access}
      scopes: ["repo:read"]
  egress: []
  tools: []
"#
    )
}

fn grant(name: &str, policy: &str, step: &str, expires: &str) -> String {
    format!(
        r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: KnobGrant
metadata:
  name: {name}
  namespace: sandboxes
spec:
  policyRef: {policy}
  knob: K2.3
  step: {step}
  owner: team-e2e
  reason: lint test
  expiresAt: "{expires}"
  approvals:
    - role: PlatformOwner
      reference: https://example.com/review/1
"#
    )
}

const IN_A_WEEK: &str = "2026-10-27T00:00:00Z";

/// An approved ceiling for `policy` that covers everything the fixtures use (ADR-0007).
fn ceiling(policy: &str, max_ttl: u32) -> String {
    format!(
        r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: ExposureCeiling
metadata:
  name: {policy}
  namespace: sandboxes
spec:
  audiences:
    - name: internal-git
      access: Write
      maxTtlSeconds: {max_ttl}
  egressHosts: []
  tools: []
  maxLeaseSeconds: 86400
  approvals:
    - role: Security
      reference: https://example.com/review/2
"#
    )
}

fn codes(r: &Report) -> Vec<&str> {
    r.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn errors(r: &Report) -> Vec<&str> {
    r.diagnostics
        .iter()
        .filter(|d| d.level == Level::Error)
        .map(|d| d.code.as_str())
        .collect()
}

#[test]
fn a_clean_repository_has_no_findings() {
    let text = format!(
        "{}---\n{}---\n{}",
        policy("agent-default", "hardened", "Read"),
        grant("pkg", "agent-default", "R1", IN_A_WEEK),
        ceiling("agent-default", 3600)
    );
    let r = lint_sources(&[src("policies.yaml", &text)], now()).unwrap();
    assert!(r.diagnostics.is_empty(), "{:#?}", r.diagnostics);
    assert_eq!(r.checked, 3);
    assert!(!r.has_errors());
}

#[test]
fn lint_codes_come_through_with_file_and_object() {
    // A grant two steps above the profile: L011, attributed to the grant in its file.
    let r = lint_sources(
        &[
            src("policy.yaml", &policy("agent-default", "hardened", "Read")),
            src(
                "grant.yaml",
                &grant("jump", "agent-default", "R2", IN_A_WEEK),
            ),
        ],
        now(),
    )
    .unwrap();
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "L011")
        .expect("an L011 finding");
    assert_eq!(d.level, Level::Error);
    assert_eq!(d.path, PathBuf::from("grant.yaml"));
    assert_eq!(d.object, "KnobGrant sandboxes/jump");
    assert!(r.has_errors());
}

#[test]
fn policy_lint_findings_are_reported() {
    // Hardened is read-only (K5.2 R0): a Write audience is L006.
    let r = lint_sources(
        &[src("p.yaml", &policy("writer", "hardened", "Write"))],
        now(),
    )
    .unwrap();
    assert!(errors(&r).contains(&"L006"), "{:?}", codes(&r));
}

#[test]
fn a_profile_in_the_files_overrides_the_built_in() {
    // A local "hardened" that allows write tokens (K5.2 R1) makes the Write audience valid.
    let local = r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxProfile
metadata:
  name: hardened
spec:
  description: local override
  dataScope: Internal
  knobs:
    K5.2: R1
"#;
    let p = policy("writer", "hardened", "Write");
    let without = lint_sources(&[src("p.yaml", &p)], now()).unwrap();
    assert!(errors(&without).contains(&"L006"));
    let with = lint_sources(&[src("p.yaml", &p), src("profile.yaml", local)], now()).unwrap();
    assert!(!errors(&with).contains(&"L006"), "{:#?}", with.diagnostics);
}

#[test]
fn documents_outside_the_group_are_ignored() {
    let other = "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: sandboxes\n---\n\
                 apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources: []\n";
    let r = lint_sources(&[src("other.yaml", other)], now()).unwrap();
    assert!(r.diagnostics.is_empty());
    assert_eq!(r.checked, 0);
    assert_eq!(r.ignored, 2);
}

#[test]
fn unparseable_yaml_is_an_input_error() {
    let err = lint_sources(&[src("broken.yaml", "kind: [unclosed\n")], now()).unwrap_err();
    assert_eq!(err.path, PathBuf::from("broken.yaml"));
}

#[test]
fn i001_invalid_object_in_the_group() {
    let unknown_kind = "apiVersion: sandbox.firestoned.io/v1alpha2\nkind: SandboxThing\n\
                        metadata:\n  name: x\n";
    let no_spec = "apiVersion: sandbox.firestoned.io/v1alpha2\nkind: SandboxPolicy\n\
                   metadata:\n  name: y\n  namespace: sandboxes\n";
    for text in [unknown_kind, no_spec] {
        let r = lint_sources(&[src("bad.yaml", text)], now()).unwrap();
        assert_eq!(errors(&r), vec!["I001"], "{text}");
    }
}

#[test]
fn i002_duplicate_object() {
    let p = policy("agent-default", "hardened", "Read");
    let r = lint_sources(&[src("a.yaml", &p), src("b.yaml", &p)], now()).unwrap();
    assert!(errors(&r).contains(&"I002"), "{:?}", codes(&r));
}

#[test]
fn i003_profile_not_found() {
    let r = lint_sources(
        &[src("p.yaml", &policy("x", "no-such-profile", "Read"))],
        now(),
    )
    .unwrap();
    assert_eq!(errors(&r), vec!["I003"]);
}

#[test]
fn i004_grant_for_a_policy_not_in_the_inputs() {
    let r = lint_sources(
        &[src("g.yaml", &grant("orphan", "missing", "R1", IN_A_WEEK))],
        now(),
    )
    .unwrap();
    assert_eq!(errors(&r), vec!["I004"]);
}

#[test]
fn i005_content_without_a_canonical_form() {
    // Unreachable under the default K4.3 ceilings, so lift them: only the canonical form refuses.
    let p = format!(
        "{}  budgets:\n    maxToolCalls: 500\n    maxWallClockSeconds: 3600\n    maxEgressBytes: 9007199254740992\n",
        policy("big", "hardened", "Read")
    );
    let ceilings = Ceilings {
        egress_bytes: [u64::MAX; 3],
        ..Ceilings::default()
    };
    let r = lint_sources_with(
        &[src("p.yaml", &p), src("c.yaml", &ceiling("big", 3600))],
        now(),
        &ceilings,
    )
    .unwrap();
    assert_eq!(errors(&r), vec!["I005"], "{:#?}", r.diagnostics);
}

#[test]
fn i006_expired_grant_fails() {
    let r = lint_sources(
        &[
            src("p.yaml", &policy("agent-default", "hardened", "Read")),
            src(
                "g.yaml",
                &grant("old", "agent-default", "R1", "2026-10-01T00:00:00Z"),
            ),
            src("c.yaml", &ceiling("agent-default", 3600)),
        ],
        now(),
    )
    .unwrap();
    assert_eq!(errors(&r), vec!["I006"]);
}

#[test]
fn i007_policy_on_a_profile_that_does_not_lint() {
    let bad_profile = r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxProfile
metadata:
  name: broken
spec:
  description: unknown knob
  dataScope: Internal
  knobs:
    K9.9: R1
"#;
    let r = lint_sources(
        &[
            src("profile.yaml", bad_profile),
            src("p.yaml", &policy("x", "broken", "Read")),
        ],
        now(),
    )
    .unwrap();
    let e = errors(&r);
    assert!(e.contains(&"L001"), "{e:?}");
    assert!(e.contains(&"I007"), "{e:?}");
    let on_policy: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.object == "SandboxPolicy sandboxes/x")
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(
        on_policy,
        vec!["I007"],
        "profile findings belong to the profile"
    );
}

#[test]
fn l015_a_policy_without_a_ceiling_fails() {
    let r = lint_sources(
        &[src("p.yaml", &policy("agent-default", "hardened", "Read"))],
        now(),
    )
    .unwrap();
    assert_eq!(errors(&r), vec!["L015"], "{:#?}", r.diagnostics);
}

#[test]
fn l014_exposure_above_the_ceiling_fails_on_the_policy() {
    // Hardened gives a 300 second token; this ceiling approves 60.
    let r = lint_sources(
        &[
            src("p.yaml", &policy("agent-default", "hardened", "Read")),
            src("c.yaml", &ceiling("agent-default", 60)),
        ],
        now(),
    )
    .unwrap();
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "L014")
        .expect("an L014 finding");
    assert_eq!(d.object, "SandboxPolicy sandboxes/agent-default");
    assert_eq!(d.path, PathBuf::from("p.yaml"));
}

#[test]
fn i008_ceiling_for_a_policy_not_in_the_inputs() {
    let r = lint_sources(&[src("c.yaml", &ceiling("missing", 3600))], now()).unwrap();
    assert_eq!(errors(&r), vec!["I008"]);
}

#[test]
fn a_retired_api_version_is_refused_by_name() {
    // ADR-0013: v1alpha1 is retired, not converted, so an old file is an input finding, not a
    // half-parsed object.
    let old = "apiVersion: sandbox.firestoned.io/v1alpha1\nkind: SandboxPolicy\n\
               metadata: { name: old, namespace: sandboxes }\n\
               spec: { profileRef: hardened, subjects: { groups: [g] } }\n";
    let r = lint_sources(&[src("old.yaml", old)], now()).unwrap();
    assert_eq!(errors(&r), vec!["I009"], "{:#?}", r.diagnostics);
    assert!(r.has_errors());
}
