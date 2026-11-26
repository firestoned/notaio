#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use serde_json::json;

use super::*;

fn spec(v: serde_json::Value) -> SandboxPolicySpec {
    serde_json::from_value(v).unwrap()
}

/// A composite on `agent` for `devs`, with the given includes and named parts.
fn composite(includes: &[&str], extra: serde_json::Value) -> SandboxPolicySpec {
    let mut v = json!({
        "profileRef": "agent",
        "subjects": { "groups": ["devs"] },
        "includes": includes.iter().map(|n| json!({ "name": n })).collect::<Vec<_>>(),
    });
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    spec(v)
}

/// A policy meant to be included: `devs` and `ops` may use it.
fn part(extra: serde_json::Value) -> SandboxPolicySpec {
    let mut v = json!({
        "profileRef": "agent",
        "subjects": { "groups": ["devs", "ops"] },
        "lease": { "maxDurationSeconds": 3600, "maxConcurrent": 2 },
    });
    for (k, val) in extra.as_object().unwrap() {
        v[k] = val.clone();
    }
    spec(v)
}

fn aud(name: &str, access: &str) -> serde_json::Value {
    json!({ "name": name, "access": access, "endpoints": [format!("{name}.example.com:443")] })
}

fn inc<'a>(name: &'a str, spec: &'a SandboxPolicySpec) -> Included<'a> {
    Included {
        name,
        uid: "uid-x",
        spec,
    }
}

fn invalid(r: Result<EffectivePolicy, ComposeError>) -> Vec<Finding> {
    match r {
        Err(ComposeError::Invalid(f)) => f,
        other => panic!("expected findings, got {other:?}"),
    }
}

fn codes(f: &[Finding]) -> Vec<&'static str> {
    f.iter().map(|f| f.code).collect()
}

#[test]
fn no_includes_is_the_policy_itself() {
    let own = composite(&[], json!({ "audiences": [aud("anthropic", "Read")] }));
    let e = compose(&own, &[]).unwrap();
    assert_eq!(e.spec, own);
    assert!(e.includes.is_empty());
    assert_eq!(e.origins.audiences, vec![None]);
}

#[test]
fn union_without_collisions_records_each_origin() {
    let own = composite(
        &["git", "pypi"],
        json!({ "audiences": [aud("anthropic", "Read")] }),
    );
    let git = part(json!({
        "audiences": [aud("github-git", "Read")],
        "egress": [{ "endpoint": "codeload.example.com:443" }],
    }));
    let pypi = part(json!({
        "tools": [{ "name": "pip", "manifestDigest": "sha256:00" }],
    }));
    let e = compose(&own, &[inc("git", &git), inc("pypi", &pypi)]).unwrap();

    let names: Vec<_> = e.spec.audiences.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["anthropic", "github-git"]);
    assert_eq!(e.origins.audiences, vec![None, Some("git".to_string())]);
    assert_eq!(e.spec.egress.len(), 1);
    assert_eq!(e.origins.egress, vec![Some("git".to_string())]);
    assert_eq!(e.spec.tools[0].name, "pip");
    assert_eq!(e.origins.tools, vec![Some("pypi".to_string())]);
    // Sandbox-wide parts are the composite's own, never an include's.
    assert_eq!(e.spec.lease, None);
    assert_eq!(e.spec.subjects.groups, ["devs"]);
    // The effective spec includes nothing: it is what gets linted and compiled.
    assert!(e.spec.includes.is_empty());
    let inc_names: Vec<_> = e.includes.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(inc_names, ["git", "pypi"]);
    assert_eq!(e.includes[0].uid, "uid-x");
    assert!(e.includes[0].content_digest.starts_with("sha256:"));
}

#[test]
fn composite_entry_replaces_an_included_one_whole() {
    let own = composite(
        &["git"],
        json!({ "audiences": [aud("github-git", "Read")] }),
    );
    let git = part(json!({ "audiences": [{
        "name": "github-git", "access": "Write", "scopes": ["repo"],
        "endpoints": ["github.example.com:443", "api.github.example.com:443"],
    }]}));
    let e = compose(&own, &[inc("git", &git)]).unwrap();
    assert_eq!(e.spec.audiences.len(), 1);
    assert_eq!(e.spec.audiences[0], own.audiences[0]);
    assert_eq!(e.origins.audiences, vec![None]);
}

#[test]
fn two_includes_with_the_same_key_are_refused_for_every_named_part() {
    let cases = [
        json!({ "audiences": [aud("pypi", "Read")] }),
        json!({ "egress": [{ "endpoint": "pypi.example.com:443" }] }),
        json!({ "tools": [{ "name": "pip", "manifestDigest": "sha256:00" }] }),
    ];
    for parts in cases {
        let own = composite(&["a", "b"], json!({}));
        let a = part(parts.clone());
        let b = part(parts.clone());
        let f = invalid(compose(&own, &[inc("a", &a), inc("b", &b)]));
        assert_eq!(codes(&f), ["L031"], "{parts}");
        assert!(f[0].message.contains("a") && f[0].message.contains("b"));
    }
}

#[test]
fn a_collision_the_composite_declares_itself_is_not_refused() {
    let own = composite(&["a", "b"], json!({ "audiences": [aud("pypi", "Read")] }));
    let a = part(json!({ "audiences": [aud("pypi", "Write")] }));
    let b = part(json!({ "audiences": [aud("pypi", "Read")] }));
    let e = compose(&own, &[inc("a", &a), inc("b", &b)]).unwrap();
    assert_eq!(e.spec.audiences, own.audiences);
}

#[test]
fn an_include_with_includes_of_its_own_is_refused() {
    let own = composite(&["mid"], json!({}));
    let mid = part(json!({ "includes": [{ "name": "leaf" }] }));
    let f = invalid(compose(&own, &[inc("mid", &mid)]));
    assert_eq!(codes(&f), ["L032"]);
    assert_eq!(f[0].path, "includes[0]");
}

#[test]
fn a_policy_that_includes_itself_is_refused() {
    let own = composite(&["self"], json!({}));
    let f = invalid(compose(&own, &[inc("self", &own)]));
    assert_eq!(codes(&f), ["L032"]);
}

#[test]
fn more_than_four_includes_are_refused() {
    let names = ["a", "b", "c", "d", "e"];
    let own = composite(&names, json!({}));
    let p = part(json!({}));
    let found: Vec<_> = names.iter().map(|n| inc(n, &p)).collect();
    let f = invalid(compose(&own, &found));
    assert_eq!(codes(&f), ["L032"]);
    assert_eq!(f[0].path, "includes");
}

#[test]
fn an_include_named_twice_is_refused() {
    let own = composite(&["a", "a"], json!({}));
    let p = part(json!({}));
    let f = invalid(compose(&own, &[inc("a", &p)]));
    assert_eq!(codes(&f), ["L032"]);
    assert_eq!(f[0].path, "includes[1]");
}

#[test]
fn includes_on_a_policy_that_is_not_published_are_refused() {
    let own = composite(&["a"], json!({ "publish": false }));
    let p = part(json!({}));
    let f = invalid(compose(&own, &[inc("a", &p)]));
    assert_eq!(codes(&f), ["L032"]);
    assert_eq!(f[0].path, "publish");
}

#[test]
fn subjects_wider_than_an_include_allows_are_refused() {
    let mut own = composite(&["a"], json!({}));
    own.subjects.groups = vec!["devs".into(), "contractors".into()];
    let p = part(json!({}));
    let f = invalid(compose(&own, &[inc("a", &p)]));
    assert_eq!(codes(&f), ["L033"]);
    assert!(f[0].message.contains("contractors"));
    assert!(f[0].message.contains("a"));
}

#[test]
fn an_include_with_no_groups_can_be_included_by_nobody() {
    let own = composite(&["a"], json!({}));
    let mut p = part(json!({}));
    p.subjects.groups.clear();
    let f = invalid(compose(&own, &[inc("a", &p)]));
    assert_eq!(codes(&f), ["L033"]);
}

#[test]
fn subjects_narrower_than_an_include_allows_are_accepted() {
    let own = composite(&["a"], json!({}));
    let p = part(json!({}));
    assert!(compose(&own, &[inc("a", &p)]).is_ok());
}

#[test]
fn a_missing_include_is_named() {
    let own = composite(&["a", "gone"], json!({}));
    let p = part(json!({}));
    match compose(&own, &[inc("a", &p)]) {
        Err(ComposeError::Missing(names)) => assert_eq!(names, ["gone"]),
        other => panic!("expected missing, got {other:?}"),
    }
}

#[test]
fn candidates_that_are_not_included_are_ignored() {
    let own = composite(&["a"], json!({}));
    let p = part(json!({ "audiences": [aud("x", "Read")] }));
    let other = part(json!({ "audiences": [aud("y", "Read")] }));
    let e = compose(&own, &[inc("a", &p), inc("other", &other)]).unwrap();
    assert_eq!(e.spec.audiences.len(), 1);
    assert_eq!(e.includes.len(), 1);
}

#[test]
fn the_order_of_includes_changes_nothing() {
    let a = part(json!({ "audiences": [aud("x", "Read")] }));
    let b = part(json!({ "audiences": [aud("y", "Read")] }));
    let ab = compose(
        &composite(&["a", "b"], json!({})),
        &[inc("a", &a), inc("b", &b)],
    )
    .unwrap();
    let ba = compose(
        &composite(&["b", "a"], json!({})),
        &[inc("b", &b), inc("a", &a)],
    )
    .unwrap();
    assert_eq!(ab.spec, ba.spec);
    assert_eq!(ab.origins, ba.origins);
    assert_eq!(ab.includes, ba.includes);
}

#[test]
fn the_content_digest_covers_only_what_an_include_contributes() {
    let own = composite(&["a"], json!({}));
    let p = part(json!({ "audiences": [aud("x", "Read")] }));
    let base = compose(&own, &[inc("a", &p)]).unwrap().includes[0]
        .content_digest
        .clone();

    let mut other_lease = p.clone();
    other_lease.lease = None;
    other_lease.subjects.groups.push("more".into());
    let same = compose(&own, &[inc("a", &other_lease)]).unwrap().includes[0]
        .content_digest
        .clone();
    assert_eq!(base, same, "lease and subjects are not contributed");

    let mut reordered = p.clone();
    reordered.audiences[0].endpoints = vec!["b.example.com:443".into(), "a.example.com:443".into()];
    let mut sorted = p.clone();
    sorted.audiences[0].endpoints = vec!["a.example.com:443".into(), "b.example.com:443".into()];
    let d1 = compose(&own, &[inc("a", &reordered)]).unwrap().includes[0]
        .content_digest
        .clone();
    let d2 = compose(&own, &[inc("a", &sorted)]).unwrap().includes[0]
        .content_digest
        .clone();
    assert_eq!(d1, d2, "order inside an entry is not content");
    assert_ne!(base, d1, "a changed endpoint is");
}

#[test]
fn findings_on_an_included_entry_name_the_include() {
    let own = composite(&["git"], json!({ "audiences": [aud("anthropic", "Read")] }));
    let git = part(json!({ "audiences": [aud("github-git", "Read")] }));
    let e = compose(&own, &[inc("git", &git)]).unwrap();
    let mut f = vec![
        Finding::err("L006", "audiences[1].ttlSeconds", "too long"),
        Finding::err("L006", "audiences[0].ttlSeconds", "too long"),
        Finding::err("L009", "lease", "too long"),
    ];
    e.attribute(&mut f);
    assert_eq!(f[0].message, "from include git: too long");
    assert_eq!(f[1].message, "too long");
    assert_eq!(f[2].message, "too long");
}
