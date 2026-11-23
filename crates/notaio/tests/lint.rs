#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

//! `notaio lint`, the engine (ADR-0006). Every input code I001 to I008 has a test here that fails
//! when its check is removed.

use std::path::PathBuf;

use jiff::Timestamp;
use notaio::lint::{lint_sources, lint_sources_with, load, Level, Report, Source};
use sandboxpolicy_core::ceilings::Ceilings;

fn now() -> Timestamp {
    "2026-10-20T00:00:00Z".parse().unwrap()
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

#[test]
fn l021_ceilings_that_disagree_about_an_audience_warn_on_each() {
    // ADR-0017, Consequences: a classification is per ceiling, so two can disagree; notaio lint
    // sees every ceiling in the repository and says so before Security signs either.
    let lab = ceiling("agent-lab", 300).replace(
        "      maxTtlSeconds: 300\n",
        "      maxTtlSeconds: 300\n      dataScope: LabOnly\n",
    );
    let r = lint_sources(
        &[
            src("p1.yaml", &policy("agent-default", "hardened", "Read")),
            src("c1.yaml", &ceiling("agent-default", 3600)),
            src("p2.yaml", &policy("agent-lab", "hardened", "Read")),
            src("c2.yaml", &lab),
        ],
        now(),
    )
    .unwrap();
    let l021: Vec<_> = r.diagnostics.iter().filter(|d| d.code == "L021").collect();
    assert_eq!(l021.len(), 2, "{:#?}", r.diagnostics);
    assert!(l021.iter().all(|d| d.level == Level::Warning));
    let objects: Vec<&str> = l021.iter().map(|d| d.object.as_str()).collect();
    assert!(
        objects.contains(&"ExposureCeiling sandboxes/agent-default"),
        "{objects:?}"
    );
    assert!(
        objects.contains(&"ExposureCeiling sandboxes/agent-lab"),
        "{objects:?}"
    );
    let on_default = l021
        .iter()
        .find(|d| d.object.ends_with("agent-default"))
        .unwrap();
    assert_eq!(on_default.path, PathBuf::from("c1.yaml"));
    assert!(
        on_default.message.contains("sandboxes/agent-lab"),
        "{}",
        on_default.message
    );
    // A warning: the repository still passes.
    assert!(!r.has_errors(), "{:#?}", r.diagnostics);
}

#[test]
fn l021_agreeing_ceilings_are_clean() {
    let r = lint_sources(
        &[
            src("p1.yaml", &policy("agent-default", "hardened", "Read")),
            src("c1.yaml", &ceiling("agent-default", 3600)),
            src("p2.yaml", &policy("agent-other", "hardened", "Read")),
            src("c2.yaml", &ceiling("agent-other", 3600)),
        ],
        now(),
    )
    .unwrap();
    assert!(!codes(&r).contains(&"L021"), "{:#?}", r.diagnostics);
}

// ADR-0020: SandboxAudience, checked when the platform audience namespace is given.

const PLATFORM_NS: &str = "sandbox-platform";

fn sandbox_audience(name: &str, ns: &str, backend: &str, endpoint: &str) -> String {
    format!(
        r#"apiVersion: sandbox.firestoned.io/v1alpha2
kind: SandboxAudience
metadata:
  name: {name}
  namespace: {ns}
spec:
  credential:
    backend: {backend}
    placement: gateway
  endpoints: ["{endpoint}"]
  maxTtlSeconds: 3600
"#
    )
}

fn lint_with_platform(text: &str) -> Report {
    notaio::lint::lint_sources_for(
        &[src("all.yaml", text)],
        now(),
        &Ceilings::default(),
        Some(PLATFORM_NS),
    )
    .unwrap()
}

/// The policy fixture grants `internal-git` and the ceiling approves it with no endpoints, so a
/// definition with no endpoint outside the ceiling is what makes the pair clean.
fn ceiling_with_endpoint(policy: &str, endpoint: &str) -> String {
    ceiling(policy, 3600).replace(
        "      maxTtlSeconds: 3600\n",
        &format!("      maxTtlSeconds: 3600\n      endpoints: [\"{endpoint}\"]\n"),
    )
}

#[test]
fn a_defined_audience_with_its_policy_is_clean() {
    let text = format!(
        "{}---\n{}---\n{}",
        policy("agent-default", "hardened", "Read"),
        ceiling_with_endpoint("agent-default", "git.example.com:443"),
        sandbox_audience("internal-git", "sandboxes", "secret", "git.example.com:443"),
    );
    let r = lint_with_platform(&text);
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
}

#[test]
fn l027_a_policy_granting_an_undefined_audience_fails() {
    let text = format!(
        "{}---\n{}",
        policy("agent-default", "hardened", "Read"),
        ceiling("agent-default", 3600),
    );
    assert!(errors(&lint_with_platform(&text)).contains(&"L027"));
}

#[test]
fn l022_is_reported_against_the_audience() {
    let text = sandbox_audience("mint", "team-a", "sts", "x.example.com:443");
    let r = lint_with_platform(&text);
    assert_eq!(errors(&r), ["L022"]);
    assert_eq!(r.diagnostics[0].object, "SandboxAudience team-a/mint");
}

#[test]
fn l014_a_definition_endpoint_outside_the_ceiling_fails() {
    let text = format!(
        "{}---\n{}---\n{}",
        policy("agent-default", "hardened", "Read"),
        ceiling_with_endpoint("agent-default", "git.example.com:443"),
        sandbox_audience(
            "internal-git",
            "sandboxes",
            "secret",
            "elsewhere.example.com:443"
        ),
    );
    assert!(errors(&lint_with_platform(&text)).contains(&"L014"));
}

/// `notaio lint` sees no status, so it refuses both sides of a collision at pull request time.
#[test]
fn l025_a_collision_is_reported_on_both() {
    let text = format!(
        "{}---\n{}",
        sandbox_audience("one", "team-a", "secret", "af.example.com:443"),
        sandbox_audience("two", "team-a", "secret", "af.example.com:443"),
    );
    assert_eq!(errors(&lint_with_platform(&text)), ["L025", "L025"]);
}

#[test]
fn i010_an_audience_without_a_namespace_fails() {
    let text = sandbox_audience("x", "team-a", "secret", "x.example.com:443")
        .replace("  namespace: team-a\n", "");
    assert_eq!(codes(&lint_with_platform(&text)), ["I010"]);
}

/// Without the platform namespace the rules cannot be applied, and lint says so rather than
/// reporting the audiences clean (rule 2 of the testing standards).
#[test]
fn i011_audiences_without_the_platform_namespace_warn_and_policies_are_unchanged() {
    let text = format!(
        "{}---\n{}---\n{}",
        policy("agent-default", "hardened", "Read"),
        ceiling("agent-default", 3600),
        sandbox_audience("mint", "team-a", "sts", "x.example.com:443"),
    );
    let r = lint_sources(&[src("all.yaml", &text)], now()).unwrap();
    assert_eq!(codes(&r), ["I011"]);
    assert_eq!(r.diagnostics[0].level, Level::Warning);
    assert!(!r.has_errors());
}

#[test]
fn an_invalid_sandbox_audience_is_i001() {
    let text = sandbox_audience("x", "team-a", "telepathy", "x.example.com:443");
    assert_eq!(codes(&lint_with_platform(&text)), ["I001"]);
}

/// A fresh directory for one test.
fn tree(name: &str) -> PathBuf {
    let d = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(unix)]
#[test]
fn a_symlink_loop_in_a_pull_request_is_an_input_error() {
    // The tree comes from the pull request under review. Following a link back to its own
    // directory recursed until the kernel's symlink limit, reading the tree forty times over.
    let d = tree("lint-symlink-loop");
    std::fs::write(d.join("policy.yaml"), "kind: ConfigMap\n").unwrap();
    std::os::unix::fs::symlink(&d, d.join("again")).unwrap();
    let Err(err) = load(std::slice::from_ref(&d)) else {
        panic!("a tree with a symlink in it was loaded");
    };
    assert_eq!(err.path, d.join("again"));
    assert!(err.message.contains("symlink"), "{}", err.message);
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_in_the_tree_is_an_input_error() {
    // Never read a file outside the tree on a pull request's say-so, and never skip one quietly:
    // a policy the linter did not read is a policy it did not check.
    let d = tree("lint-symlink-file");
    let outside = tree("lint-symlink-file-outside").join("secret.yaml");
    std::fs::write(&outside, "kind: ConfigMap\n").unwrap();
    std::os::unix::fs::symlink(&outside, d.join("policy.yaml")).unwrap();
    let Err(err) = load(std::slice::from_ref(&d)) else {
        panic!("a tree with a symlink in it was loaded");
    };
    assert_eq!(err.path, d.join("policy.yaml"));
}

#[cfg(unix)]
#[test]
fn a_path_named_on_the_command_line_is_followed() {
    // The operator chose it: only links found while walking are refused.
    let real = tree("lint-named-real");
    std::fs::write(real.join("policy.yaml"), "kind: ConfigMap\n").unwrap();
    let link = tree("lint-named").join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let sources = load(std::slice::from_ref(&link)).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].path, link.join("policy.yaml"));
}
