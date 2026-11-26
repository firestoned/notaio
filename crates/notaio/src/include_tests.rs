// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::unwrap_used, clippy::expect_used)]

use sandboxpolicy_api::IncludeStatus;
use serde_json::json;

use super::*;

fn policy(ns: &str, name: &str, includes: &[&str]) -> SandboxPolicy {
    serde_json::from_value(json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
        "kind": "SandboxPolicy",
        "metadata": { "name": name, "namespace": ns },
        "spec": {
            "profileRef": "hardened",
            "includes": includes.iter().map(|n| json!({ "name": n })).collect::<Vec<_>>(),
        }
    }))
    .unwrap()
}

fn names(refs: &[ObjectRef<SandboxPolicy>]) -> Vec<(String, String)> {
    let mut v: Vec<_> = refs
        .iter()
        .map(|r| (r.namespace.clone().unwrap_or_default(), r.name.clone()))
        .collect();
    v.sort();
    v
}

#[test]
fn included_by_lists_the_composites_in_the_namespace_sorted() {
    let all = [
        policy("a", "zeta", &["git"]),
        policy("a", "alpha", &["git", "pypi"]),
        policy("a", "other", &["pypi"]),
        policy("b", "elsewhere", &["git"]),
        policy("a", "git", &[]),
    ];
    assert_eq!(included_by(&all, "a", "git"), ["alpha", "zeta"]);
    assert!(included_by(&all, "a", "nobody").is_empty());
}

#[test]
fn a_change_to_an_include_requeues_its_composites_only_in_its_namespace() {
    let all = [
        policy("a", "dev", &["git"]),
        policy("a", "ops", &["git"]),
        policy("a", "unrelated", &[]),
        policy("b", "dev", &["git"]),
    ];
    let git = policy("a", "git", &[]);
    assert_eq!(
        names(&related(&all, &git)),
        [("a".into(), "dev".into()), ("a".into(), "ops".into())]
    );
}

#[test]
fn a_change_to_a_composite_requeues_what_it_includes_now_and_before() {
    // Its spec names git; its status still records pypi, which it no longer includes, and which
    // must be told so (its includedBy shrinks).
    let mut dev = policy("a", "dev", &["git"]);
    dev.status = Some(sandboxpolicy_api::SandboxPolicyStatus {
        includes: vec![IncludeStatus {
            name: "pypi".into(),
            uid: String::new(),
            content_digest: "sha256:00".into(),
        }],
        ..Default::default()
    });
    assert_eq!(
        names(&related(&[], &dev)),
        [("a".into(), "git".into()), ("a".into(), "pypi".into())]
    );
}

#[test]
fn a_policy_is_never_its_own_related_object_and_none_is_listed_twice() {
    let me = policy("a", "me", &["me", "git"]);
    let all = [me.clone(), policy("a", "git", &["me"])];
    assert_eq!(names(&related(&all, &me)), [("a".into(), "git".into())]);
}

#[test]
fn empty_include_lists_are_written_so_a_merge_patch_clears_them() {
    // Status goes out as a JSON merge patch, where an absent field means "unchanged": a composite
    // that stops including, or an include nobody includes any more, must write the empty list.
    let v = serde_json::to_value(sandboxpolicy_api::SandboxPolicyStatus::default()).unwrap();
    assert_eq!(v["includes"], json!([]));
    assert_eq!(v["includedBy"], json!([]));
}
