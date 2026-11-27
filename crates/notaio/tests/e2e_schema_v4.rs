// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: bundle schema v4 from the deployed controller (ADR-0026, roadmap 05 phase 4).
//!
//! Run with `make kind-e2e-schema-v4`, which starts the controller with
//! `NOTAIO_BUNDLE_SCHEMA=v4` and always unsets it again. A composite is published as v4 with its
//! includes and the include behind each entry; a plain policy signs its groups, sorted; and the
//! operator's switch is turned to `v3` and back to `v4` while the bundle's version keeps climbing,
//! which is what lets a consumer that reads both follow a switch either way and a rollback.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use jiff::Timestamp;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::{Api, ResourceExt};
use sandboxpolicy_api::{ExposureCeiling, SandboxPolicy};
use sandboxpolicy_bundle::{
    verify_bundle, BundlePayload, Envelope, SchemaVersion, VerifyOptions, PAYLOAD_TYPE_V3,
    PAYLOAD_TYPE_V4,
};
use serde_json::{json, Value};

use common::*;

const GROUP_B: &str = "00000000-0000-0000-0000-00000000000b";
const GROUP_A: &str = "00000000-0000-0000-0000-00000000000a";
const TOOL_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const DEPLOYMENT: &str = "notaio";
const CONTAINER: &str = "notaio";
const ENV_BUNDLE_SCHEMA: &str = "NOTAIO_BUNDLE_SCHEMA";

async fn create<K>(api: &Api<K>, v: Value)
where
    K: kube::Resource + Clone + serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug,
{
    let obj: K = serde_json::from_value(v).expect("fixture");
    api.create(&PostParams::default(), &obj)
        .await
        .expect("create");
}

/// The envelope and its verified payload once the policy's ConfigMap holds a bundle of a version
/// above `after`.
async fn published_after(
    cms: &Api<ConfigMap>,
    policy: &str,
    after: u64,
) -> (Envelope, BundlePayload) {
    let name = bundle_object_name(policy);
    let env: Envelope = wait_for(&format!("ConfigMap {name} above version {after}"), || {
        let cms = cms.clone();
        let name = name.clone();
        async move {
            let cm = cms.get_opt(&name).await.ok()??;
            let v = cm.annotations().get(ANN_VERSION)?.parse::<u64>().ok()?;
            if v <= after {
                return None;
            }
            serde_json::from_str(cm.data?.get(BUNDLE_KEY)?).ok()
        }
    })
    .await;
    let payload = verify_bundle(&env, &trusted_keys(), &VerifyOptions::at(Timestamp::now()))
        .expect("the published bundle verifies");
    (env, payload)
}

/// Sets the controller's schema and waits until every replica runs with it.
async fn switch_schema(deployments: &Api<Deployment>, schema: &str) {
    let patch = json!({ "spec": { "template": { "spec": { "containers": [
        { "name": CONTAINER, "env": [{ "name": ENV_BUNDLE_SCHEMA, "value": schema }] }
    ] } } } });
    deployments
        .patch(
            DEPLOYMENT,
            &PatchParams::default(),
            &Patch::Strategic(patch),
        )
        .await
        .expect("set the bundle schema");
    wait_for(&format!("the controller rolled out with {schema}"), || {
        let deployments = deployments.clone();
        async move {
            let d = deployments.get(DEPLOYMENT).await.ok()?;
            let want = d.spec.replicas.unwrap_or(1);
            let s = d.status.as_ref()?;
            let done = s.observed_generation >= d.metadata.generation
                && s.updated_replicas == Some(want)
                && s.available_replicas == Some(want)
                && s.replicas == Some(want);
            done.then_some(())
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-schema-v4"]
async fn the_controller_emits_v4_and_switches_back_and_forth() {
    let client = client().await;
    let ns = policy_namespace();
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    let ceilings: Api<ExposureCeiling> = Api::namespaced(client.clone(), &ns);
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), &controller_namespace());

    // 1. A composite under v4: its includes as its status records them, and `from` on the entry
    //    an include supplied only.
    let dev = unique_name("v4-composite");
    let tools = format!("{dev}-tools");
    create(
        &policies,
        json!({
            "apiVersion": "sandbox.firestoned.io/v1alpha2",
            "kind": "SandboxPolicy",
            "metadata": { "name": tools, "namespace": ns },
            "spec": {
                "profileRef": "hardened",
                "publish": false,
                "subjects": { "groups": [GROUP_A] },
                "tools": [{ "name": "linter", "manifestDigest": TOOL_DIGEST }]
            }
        }),
    )
    .await;
    let mut ceiling = hardened_ceiling(&dev, &ns);
    ceiling["spec"]["tools"] = json!(["linter"]);
    create(&ceilings, ceiling).await;
    let mut composite = hardened_policy(&dev, &ns);
    composite["spec"]["subjects"]["groups"] = json!([GROUP_A]);
    composite["spec"]["includes"] = json!([{ "name": tools }]);
    create(&policies, composite).await;

    let (env, p) = published_after(&cms, &dev, 0).await;
    assert_eq!(env.payload_type, PAYLOAD_TYPE_V4);
    assert_eq!(p.schema_version(), Some(SchemaVersion::V4));
    let status = policies.get(&dev).await.unwrap().status.unwrap();
    let signed: Vec<_> = p
        .content
        .includes
        .iter()
        .map(|i| (i.name.clone(), i.uid.clone(), i.content_digest.clone()))
        .collect();
    let recorded: Vec<_> = status
        .includes
        .iter()
        .map(|i| (i.name.clone(), i.uid.clone(), i.content_digest.clone()))
        .collect();
    assert_eq!(signed, recorded);
    assert_eq!(p.content.tools[0].from.as_deref(), Some(tools.as_str()));
    assert!(p.content.audiences.iter().all(|a| a.from.is_none()));

    // 2. A plain policy signs its groups, sorted.
    let plain = unique_name("v4-plain");
    create(&ceilings, hardened_ceiling(&plain, &ns)).await;
    let mut policy = hardened_policy(&plain, &ns);
    policy["spec"]["subjects"]["groups"] = json!([GROUP_B, GROUP_A]);
    create(&policies, policy).await;
    let (_, v4) = published_after(&cms, &plain, 0).await;
    assert_eq!(
        v4.content.subjects.clone().unwrap().groups,
        [GROUP_A, GROUP_B]
    );

    // 3. The operator turns v4 off: the next bundle is v3, signs no groups, and is a new version.
    switch_schema(&deployments, "v3").await;
    let (env, v3) = published_after(&cms, &plain, v4.meta.version).await;
    assert_eq!(env.payload_type, PAYLOAD_TYPE_V3);
    assert_eq!(v3.schema_version(), Some(SchemaVersion::V3));
    assert!(v3.content.subjects.is_none());

    // 4. And on again: v4, a version above both.
    switch_schema(&deployments, "v4").await;
    let (env, again) = published_after(&cms, &plain, v3.meta.version).await;
    assert_eq!(env.payload_type, PAYLOAD_TYPE_V4);
    assert_eq!(again.content.subjects.unwrap().groups, [GROUP_A, GROUP_B]);

    for name in [&dev, &tools, &plain] {
        policies
            .delete(name, &DeleteParams::default())
            .await
            .expect("delete policy");
    }
    for name in [&dev, &plain] {
        ceilings
            .delete(name, &DeleteParams::default())
            .await
            .expect("delete ceiling");
    }
}
