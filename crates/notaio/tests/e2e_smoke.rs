// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: a read-only smoke test of a running installation (roadmap 01, phase C).
//!
//! Run with `make cluster-smoke` against any cluster notaio is installed on, including one with
//! GitOps-only admission, where the other suites cannot create their fixtures. It creates nothing
//! and deletes nothing: the one write it attempts is a server-side dry run, which admission sees
//! and nothing persists.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chrono::Utc;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{ListParams, Patch, PatchParams};
use kube::{Api, ResourceExt};
use sandboxpolicy_api::SandboxPolicy;
use sandboxpolicy_bundle::{verify_bundle, Envelope, VerifyOptions};
use serde_json::json;

use common::*;

/// The sample `name` in a Prometheus exposition.
fn sample(metrics: &str, name: &str) -> Option<f64> {
    metrics
        .lines()
        .find_map(|l| l.strip_prefix(name)?.strip_prefix(' ')?.parse().ok())
}

#[tokio::test]
#[ignore = "needs an installed notaio: make cluster-smoke"]
async fn the_controller_is_healthy_and_leads_once() {
    let client = client().await;
    let ns = controller_namespace();

    let d: Api<Deployment> = Api::namespaced(client.clone(), &ns);
    let d = d.get("notaio").await.expect("deployment/notaio");
    let want = d.spec.as_ref().and_then(|s| s.replicas).unwrap_or(1);
    let status = d.status.expect("deployment status");
    assert_eq!(
        status.available_replicas,
        Some(want),
        "not every replica is available"
    );
    assert_eq!(
        status.updated_replicas,
        Some(want),
        "a rollout is unfinished"
    );

    let pods = running_controller_pods(&client).await;
    assert_eq!(pods.len() as i32, want, "{pods:?}");
    let leases: Api<Lease> = Api::namespaced(client.clone(), &ns);
    let holder = leases
        .get("notaio")
        .await
        .expect("the leader Lease")
        .spec
        .and_then(|s| s.holder_identity)
        .expect("a Lease holder");
    assert!(
        pods.contains(&holder),
        "the Lease holder {holder} is not a running replica"
    );

    let mut leaders = 0.0;
    for pod in &pods {
        let (code, _) = controller_get_from(&client, pod, "/readyz").await;
        assert_eq!(code, 200, "{pod} is not ready");
        let (code, metrics) = controller_get_from(&client, pod, "/metrics").await;
        assert_eq!(code, 200, "{pod}: /metrics");
        leaders += sample(&metrics, "notaio_leader").expect("notaio_leader");
        assert_eq!(
            sample(&metrics, "notaio_relaxations_past_expiry"),
            Some(0.0),
            "{pod}: an active relaxation is past its expiry"
        );
        for check in [
            "no-ceiling",
            "error-findings",
            "expired-relaxation",
            "already-expired",
        ] {
            assert_eq!(
                sample(
                    &metrics,
                    &format!("notaio_fail_open_total{{check=\"{check}\"}}")
                ),
                Some(0.0),
                "{pod}: the fail-open guard refused a bundle ({check})"
            );
        }
    }
    assert_eq!(leaders, 1.0, "exactly one replica reports it leads");
}

#[tokio::test]
#[ignore = "needs an installed notaio: make cluster-smoke"]
async fn every_policy_is_published_and_its_bundle_verifies() {
    let client = client().await;
    let keys = trusted_keys();
    let policies: Api<SandboxPolicy> = Api::all(client.clone());
    let all = policies
        .list(&ListParams::default())
        .await
        .expect("list policies");
    for p in &all.items {
        let ns = p.namespace().expect("a namespaced policy");
        let name = p.name_any();
        let status = p.status.clone().unwrap_or_default();
        assert!(
            condition_true(&status.conditions, "Ready", Some("Published")),
            "{ns}/{name} is not published: {:?}",
            status.conditions.iter().find(|c| c.type_ == "Ready")
        );
        let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
        let cm = cms
            .get(&bundle_object_name(&name))
            .await
            .unwrap_or_else(|e| panic!("{ns}/{name}: no bundle ConfigMap: {e}"));
        let env: Envelope =
            serde_json::from_str(&cm.data.unwrap_or_default()[BUNDLE_KEY]).expect("an envelope");
        let payload = verify_bundle(&env, &keys, &VerifyOptions::at(Utc::now()))
            .unwrap_or_else(|e| panic!("{ns}/{name}: the bundle does not verify: {e}"));
        assert_eq!(
            Some(payload.meta.version),
            status.bundle_version,
            "{ns}/{name}"
        );
        assert_eq!(
            Some(&payload.content_digest),
            status.bundle_digest.as_ref(),
            "{ns}/{name}"
        );
        assert_eq!(payload.content.policy.namespace, ns);
        assert_eq!(payload.content.policy.name, name);
        assert_eq!(
            Some(payload.content.policy.uid.as_str()),
            p.metadata.uid.as_deref()
        );
        println!("{ns}/{name}: version {} verifies", payload.meta.version);
    }
}

#[tokio::test]
#[ignore = "needs an installed notaio: make cluster-smoke"]
async fn a_direct_edit_is_denied() {
    // GitOps-only admission (T8.2). A server-side dry run goes through admission and persists
    // nothing, so this checks the lock without touching a policy.
    let client = client().await;
    let policies: Api<SandboxPolicy> = Api::all(client.clone());
    let all = policies
        .list(&ListParams::default())
        .await
        .expect("list policies");
    let Some(p) = all.items.first() else {
        println!("no SandboxPolicy installed: nothing to try a direct edit on");
        return;
    };
    let ns = p.namespace().expect("namespaced");
    let api: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    let result = api
        .patch(
            &p.name_any(),
            &PatchParams::default().dry_run(),
            &Patch::Merge(json!({ "metadata": { "labels": { "smoke": "direct-edit" } } })),
        )
        .await;
    match result {
        Err(kube::Error::Api(s)) if s.code == 403 => {
            assert!(
                s.message.contains("sandbox-policy-gitops-only"),
                "denied, but not by the GitOps-only admission policy: {}",
                s.message
            );
        }
        Ok(_) => panic!(
            "a direct edit of {ns}/{} was admitted: is deploy/admission/gitops-only.yaml applied?",
            p.name_any()
        ),
        Err(e) => panic!("a direct edit failed, but not by admission: {e}"),
    }
}
