// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: a policy composed of others (ADR-0022), through the deployed controller.
//!
//! Run with `make kind-e2e-compose`. A composite that includes two `publish: false` policies is
//! published as one verified bundle with both includes' grants; the includes publish nothing and
//! report who includes them; reordering `includes` changes nothing; an include widened past the
//! composite's ceiling stops its re-signing; and a deleted include does the same. Each change to
//! an include must reach the composite through the controller's watch, well inside the bundle
//! lifetime, which is what the waits below bound.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use jiff::Timestamp;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::{Api, ResourceExt};
use sandboxpolicy_api::{ExposureCeiling, SandboxPolicy};
use sandboxpolicy_bundle::{verify_bundle, Envelope, VerifyOptions};
use serde_json::{json, Value};

use common::*;

const GROUP: &str = "00000000-0000-0000-0000-000000000000";
const TOOL_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// A `publish: false` policy on `hardened` with the given named parts.
fn include(name: &str, ns: &str, parts: Value) -> Value {
    let mut spec = json!({
        "profileRef": "hardened",
        "publish": false,
        "subjects": { "groups": [GROUP] },
    });
    for (k, v) in parts.as_object().unwrap() {
        spec[k] = v.clone();
    }
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
        "kind": "SandboxPolicy",
        "metadata": { "name": name, "namespace": ns },
        "spec": spec,
    })
}

fn composite(name: &str, ns: &str, includes: &[&str]) -> Value {
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
        "kind": "SandboxPolicy",
        "metadata": { "name": name, "namespace": ns },
        "spec": {
            "profileRef": "hardened",
            "subjects": { "groups": [GROUP] },
            "includes": includes.iter().map(|n| json!({ "name": n })).collect::<Vec<_>>(),
        }
    })
}

/// The composite's ceiling: the git include's audience and the tool include's tool, nothing else.
fn ceiling(name: &str, ns: &str, tool: &str) -> Value {
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
        "kind": "ExposureCeiling",
        "metadata": { "name": name, "namespace": ns },
        "spec": {
            "audiences": [
                { "name": "internal-git", "access": "Read", "maxTtlSeconds": 300,
                  "endpoints": ["git.foo.io:443"] }
            ],
            "egressHosts": [],
            "tools": [tool],
            "maxLeaseSeconds": 14400,
            "approvals": [
                { "role": "Security", "reference": "https://example.com/review/2" }
            ]
        }
    })
}

fn ready_reason(p: &SandboxPolicy) -> Option<(String, String)> {
    let c = p
        .status
        .as_ref()?
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")?;
    Some((c.status.clone(), c.reason.clone()))
}

/// The policy once its `Ready` condition has `reason` for its current generation.
async fn wait_reason(policies: &Api<SandboxPolicy>, name: &str, reason: &str) -> SandboxPolicy {
    wait_for(
        &format!("SandboxPolicy {name} Ready reason {reason}"),
        || {
            let policies = policies.clone();
            let name = name.to_string();
            let reason = reason.to_string();
            async move {
                let p = policies.get(&name).await.ok()?;
                let current = p.status.as_ref()?.observed_generation == p.metadata.generation;
                (current && ready_reason(&p)?.1 == reason).then_some(p)
            }
        },
    )
    .await
}

async fn published(cms: &Api<ConfigMap>, policy: &str, version: u64) -> Envelope {
    let name = bundle_object_name(policy);
    let cm = wait_for(&format!("ConfigMap {name} at version {version}"), || {
        let cms = cms.clone();
        let name = name.clone();
        async move {
            let cm = cms.get_opt(&name).await.ok()??;
            let v = cm.annotations().get(ANN_VERSION)?.parse::<u64>().ok()?;
            (v == version).then_some(cm)
        }
    })
    .await;
    serde_json::from_str(cm.data.unwrap().get(BUNDLE_KEY).unwrap()).unwrap()
}

async fn create<K>(api: &Api<K>, v: Value)
where
    K: kube::Resource + Clone + serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug,
{
    let obj: K = serde_json::from_value(v).expect("fixture");
    api.create(&PostParams::default(), &obj)
        .await
        .expect("create");
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-compose"]
async fn a_composite_is_one_bundle_bounded_by_its_own_ceiling() {
    let client = client().await;
    let ns = policy_namespace();
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    let ceilings: Api<ExposureCeiling> = Api::namespaced(client.clone(), &ns);
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let keys = trusted_keys();

    let dev = unique_name("compose");
    let git = format!("{dev}-git");
    let tool = format!("{dev}-tool");
    let tool_name = "linter";

    create(
        &policies,
        include(
            &git,
            &ns,
            json!({ "audiences": [{ "name": "internal-git", "access": "Read",
                "scopes": ["repo:read"], "endpoints": ["git.foo.io:443"] }] }),
        ),
    )
    .await;
    create(
        &policies,
        include(
            &tool,
            &ns,
            json!({ "tools": [{ "name": tool_name, "manifestDigest": TOOL_DIGEST }] }),
        ),
    )
    .await;
    create(&ceilings, ceiling(&dev, &ns, tool_name)).await;
    create(&policies, composite(&dev, &ns, &[&git, &tool])).await;

    // 1. The composite publishes one bundle with both includes' grants, under its own ceiling.
    let p1 = wait_reason(&policies, &dev, "Published").await;
    let s1 = p1.status.clone().unwrap();
    let v1 = s1.bundle_version.unwrap();
    let payload = verify_bundle(
        &published(&cms, &dev, v1).await,
        &keys,
        &VerifyOptions::at(Timestamp::now()),
    )
    .expect("the composite's bundle verifies");
    let audiences: Vec<_> = payload.content.audiences.iter().map(|a| &a.name).collect();
    assert_eq!(audiences, ["internal-git"]);
    let tools: Vec<_> = payload.content.tools.iter().map(|t| &t.name).collect();
    assert_eq!(tools, [tool_name]);
    let mut included: Vec<_> = s1.includes.iter().map(|i| i.name.clone()).collect();
    included.sort();
    let mut expected = vec![git.clone(), tool.clone()];
    expected.sort();
    assert_eq!(included, expected);

    // The includes publish nothing, need no ceiling, and name who includes them.
    for name in [&git, &tool] {
        let p = wait_reason(&policies, name, "NotPublished").await;
        assert_eq!(ready_reason(&p).unwrap().0, "True");
        wait_for(&format!("{name} includedBy {dev}"), || {
            let policies = policies.clone();
            let name = name.clone();
            let dev = dev.clone();
            async move {
                let p = policies.get(&name).await.ok()?;
                (p.status?.included_by == std::slice::from_ref(&dev)).then_some(())
            }
        })
        .await;
        assert!(
            cms.get_opt(&bundle_object_name(name))
                .await
                .unwrap()
                .is_none(),
            "{name} has publish: false and must have no bundle"
        );
    }

    // 2. Reordering `includes` is a new generation and the same content: same digest, no new
    //    version.
    policies
        .patch(
            &dev,
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "includes": [{ "name": tool }, { "name": git }] } })),
        )
        .await
        .expect("reorder includes");
    let p2 = wait_reason(&policies, &dev, "Published").await;
    let s2 = p2.status.unwrap();
    assert_eq!(s2.bundle_digest, s1.bundle_digest);
    assert_eq!(s2.bundle_version, Some(v1));

    // 3. The include's owner widens it past the composite's ceiling: the composite stops
    //    re-signing (L014) and its last bundle stays where consumers read it.
    policies
        .patch(
            &git,
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "audiences": [{ "name": "internal-git",
                "access": "Read", "scopes": ["repo:read"],
                "endpoints": ["git.foo.io:443", "git-mirror.foo.io:443"] }] } })),
        )
        .await
        .expect("widen the include");
    wait_reason(&policies, &dev, "ExposureAboveCeiling").await;
    let still = cms
        .get(&bundle_object_name(&dev))
        .await
        .expect("the last bundle is kept");
    assert_eq!(
        still.annotations().get(ANN_VERSION).map(String::as_str),
        Some(v1.to_string().as_str())
    );

    // 4. The include is deleted: the composite reports it by name and compiles nothing.
    policies
        .delete(&git, &DeleteParams::default())
        .await
        .expect("delete include");
    let gone = wait_reason(&policies, &dev, "IncludeNotFound").await;
    let msg = &gone
        .status
        .unwrap()
        .conditions
        .into_iter()
        .find(|c| c.type_ == "Ready")
        .unwrap()
        .message;
    assert!(msg.contains(&git), "{msg}");
    // The tool include's includedBy still names the composite, which still includes it.
    let t = policies.get(&tool).await.unwrap();
    assert_eq!(t.status.unwrap().included_by, std::slice::from_ref(&dev));

    // Cleanup. The composite's bundle ConfigMap is owned by it and goes with it.
    for name in [&dev, &tool] {
        policies
            .delete(name, &DeleteParams::default())
            .await
            .expect("delete policy");
    }
    ceilings
        .delete(&dev, &DeleteParams::default())
        .await
        .expect("delete ceiling");
}
