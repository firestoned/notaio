// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: a SandboxAudience goes in, a signed audience bundle comes out (ADR-0020).
//!
//! Run with `make kind-e2e-audience`, which turns audiences on in the deployed controller
//! (`NOTAIO_AUDIENCE_PLATFORM_NAMESPACE`) for the run and off again after it. Proves what an offline
//! test cannot: the API server accepts the CRD and the objects, the controller publishes
//! `<audience>-audience` signed by OpenBao, a consumer verifies it with the audience verifier only,
//! a finding keeps the last valid bundle, deleting the audience deletes its bundle, and a policy
//! that grants an undefined audience publishes nothing (L027).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use jiff::Timestamp;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::{Api, ResourceExt};
use sandboxpolicy_api::{SandboxAudience, SandboxPolicy};
use sandboxpolicy_bundle::{
    verify_audience_bundle, verify_bundle, AudienceBundlePayload, BundleError, Envelope,
    VerifyOptions, AUDIENCE_PAYLOAD_TYPE,
};
use serde_json::{json, Value};

use common::*;

/// The published audience contract, restated on purpose: these live in
/// `crates/notaio/src/audience.rs` and a consumer depends on them.
const AUDIENCE_BUNDLE_KEY: &str = "audience.dsse.json";

fn audience_object_name(audience: &str) -> String {
    format!("{audience}-audience")
}

fn team_audience(name: &str, namespace: &str, backend: &str) -> Value {
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
        "kind": "SandboxAudience",
        "metadata": { "name": name, "namespace": namespace },
        "spec": {
            "credential": {
                "backend": backend,
                "placement": "gateway",
                "format": "basic",
                "source": { "secretRef": { "name": "af-svc", "tokenKey": "token", "usernameKey": "username" } }
            },
            "endpoints": [format!("{name}.example.com:443")],
            "rules": [{ "method": "GET", "path": "/v2/**" }],
            "maxTtlSeconds": 900
        }
    })
}

/// The audience's published envelope, once its version annotation reaches `version`.
async fn published_envelope(cms: &Api<ConfigMap>, audience: &str, version: u64) -> Envelope {
    let name = audience_object_name(audience);
    let cm = wait_for(
        &format!("ConfigMap {name} at version {version}"),
        || async {
            let cm = cms.get_opt(&name).await.ok().flatten()?;
            let v: u64 = cm.annotations().get(ANN_VERSION)?.parse().ok()?;
            (v >= version).then_some(cm)
        },
    )
    .await;
    assert_eq!(
        cm.labels()
            .get("app.kubernetes.io/managed-by")
            .map(String::as_str),
        Some(MANAGED_BY)
    );
    let raw = cm
        .data
        .as_ref()
        .and_then(|d| d.get(AUDIENCE_BUNDLE_KEY))
        .expect("the ConfigMap holds audience.dsse.json");
    serde_json::from_str(raw).expect("an envelope")
}

async fn wait_ready(api: &Api<SandboxAudience>, name: &str, above: u64) -> SandboxAudience {
    wait_for(
        &format!("SandboxAudience {name} Ready above version {above}"),
        || async {
            let a = api.get(name).await.ok()?;
            let s = a.status.as_ref()?;
            let ready = s
                .conditions
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True");
            (ready && s.bundle_version? > above).then_some(a)
        },
    )
    .await
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-audience"]
async fn a_team_audience_is_published_verified_kept_on_a_finding_and_deleted_with_its_object() {
    let client = client().await;
    let ns = policy_namespace();
    let keys = trusted_keys();
    let audiences: Api<SandboxAudience> = Api::namespaced(client.clone(), &ns);
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let name = unique_name("af");

    let obj: SandboxAudience =
        serde_json::from_value(team_audience(&name, &ns, "secret")).expect("a SandboxAudience");
    audiences
        .create(&PostParams::default(), &obj)
        .await
        .expect("create SandboxAudience");

    // 1. Published and verifiable with the audience verifier and OpenBao's key alone.
    let a1 = wait_ready(&audiences, &name, 0).await;
    let v1 = a1.status.as_ref().unwrap().bundle_version.unwrap();
    let env = published_envelope(&cms, &name, v1).await;
    assert_eq!(env.payload_type, AUDIENCE_PAYLOAD_TYPE);
    let payload: AudienceBundlePayload =
        verify_audience_bundle(&env, &keys, &VerifyOptions::at(Timestamp::now()))
            .expect("the audience bundle verifies");
    assert_eq!(payload.content.audience.namespace, ns);
    assert_eq!(payload.content.audience.name, name);
    assert_eq!(
        Some(payload.content.audience.uid.as_str()),
        a1.uid().as_deref()
    );
    assert_eq!(payload.content.definition, a1.spec.definition);
    assert_eq!(
        Some(&payload.content_digest),
        a1.status.as_ref().unwrap().bundle_digest.as_ref()
    );
    // Never accepted as a policy bundle.
    let err = verify_bundle(&env, &keys, &VerifyOptions::at(Timestamp::now())).unwrap_err();
    assert!(matches!(err, BundleError::PayloadType(_)), "{err}");
    // Owned by the audience, so deleting it deletes the bundle.
    let cm = cms.get(&audience_object_name(&name)).await.unwrap();
    let owner = cm.owner_references().first().expect("an owner reference");
    assert_eq!(owner.kind, "SandboxAudience");
    assert_eq!(Some(owner.uid.as_str()), a1.uid().as_deref());

    // 2. A finding (L022) publishes nothing new: Ready turns false, the last bundle stays.
    let patch = json!({ "spec": { "credential": { "backend": "sts", "source": null } } });
    audiences
        .patch(&name, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .expect("patch to a minting backend");
    let refused = wait_for(&format!("{name} refused with L022"), || async {
        let a = audiences.get(&name).await.ok()?;
        let valid = a
            .status
            .as_ref()?
            .conditions
            .iter()
            .find(|c| c.type_ == "Valid")?
            .clone();
        (valid.status == "False" && valid.message.contains("L022")).then_some(a)
    })
    .await;
    assert_eq!(refused.status.as_ref().unwrap().bundle_version, Some(v1));
    let cm = cms.get(&audience_object_name(&name)).await.unwrap();
    assert_eq!(
        cm.annotations().get(ANN_VERSION).map(String::as_str),
        Some(v1.to_string().as_str())
    );

    // 3. Deleting the audience deletes its bundle (garbage collection through the owner).
    audiences
        .delete(&name, &DeleteParams::default())
        .await
        .expect("delete SandboxAudience");
    wait_for(
        &format!("ConfigMap {} deleted", audience_object_name(&name)),
        || async {
            cms.get_opt(&audience_object_name(&name))
                .await
                .ok()
                .and_then(|cm| cm.is_none().then_some(()))
        },
    )
    .await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-audience"]
async fn a_policy_granting_an_undefined_audience_publishes_nothing() {
    let client = client().await;
    let ns = policy_namespace();
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    let ceilings: Api<sandboxpolicy_api::ExposureCeiling> = Api::namespaced(client.clone(), &ns);
    let name = unique_name("undefined");

    ceilings
        .create(
            &PostParams::default(),
            &serde_json::from_value(hardened_ceiling(&name, &ns)).unwrap(),
        )
        .await
        .expect("create ExposureCeiling");
    // Grants internal-git, which no SandboxAudience defines.
    policies
        .create(
            &PostParams::default(),
            &serde_json::from_value(hardened_policy(&name, &ns)).unwrap(),
        )
        .await
        .expect("create SandboxPolicy");

    let p = wait_for(&format!("{name} invalid with L027"), || async {
        let p = policies.get(&name).await.ok()?;
        let valid = p
            .status
            .as_ref()?
            .conditions
            .iter()
            .find(|c| c.type_ == "Valid")?
            .clone();
        (valid.status == "False" && valid.message.contains("L027")).then_some(p)
    })
    .await;
    assert_eq!(p.status.unwrap().bundle_version, None);
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    assert!(cms
        .get_opt(&bundle_object_name(&name))
        .await
        .unwrap()
        .is_none());
}
