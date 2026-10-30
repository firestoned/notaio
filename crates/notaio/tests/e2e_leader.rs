// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: two replicas, one leader (ADR-0011).
//!
//! Run with `make kind-e2e-leader`. Exactly one replica holds the Lease and says so in its metrics;
//! when its pod is deleted, the other takes over and publishes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{DeleteParams, PostParams};
use kube::{Api, Client};
use sandboxpolicy_api::{ExposureCeiling, SandboxPolicy};

use common::*;

const LEASE: &str = "notaio";

/// The Lease's holder, once it names one of the running replicas.
async fn holder(client: &Client) -> String {
    let leases: Api<Lease> = Api::namespaced(client.clone(), &controller_namespace());
    wait_for("the Lease to name a running replica", || {
        let leases = leases.clone();
        async move {
            let h = leases.get_opt(LEASE).await.ok()??.spec?.holder_identity?;
            running_controller_pods(client)
                .await
                .contains(&h)
                .then_some(h)
        }
    })
    .await
}

fn leader_gauge(metrics: &str) -> Option<f64> {
    metrics
        .lines()
        .find_map(|l| l.strip_prefix("notaio_leader ")?.parse().ok())
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-leader"]
async fn one_replica_leads_and_another_takes_over() {
    let client = client().await;

    // 1. Two replicas, both ready: a follower is ready too, so a rollout completes.
    let pods = wait_for("two running replicas", || async {
        let p = running_controller_pods(&client).await;
        (p.len() == 2).then_some(p)
    })
    .await;

    // 2. Exactly one leads, and the metrics agree with the Lease.
    let leader = holder(&client).await;
    for pod in &pods {
        let (code, _) = controller_get_from(&client, pod, "/readyz").await;
        assert_eq!(code, 200, "{pod} is not ready");
        let (_, metrics) = controller_get_from(&client, pod, "/metrics").await;
        let want = if *pod == leader { 1.0 } else { 0.0 };
        assert_eq!(leader_gauge(&metrics), Some(want), "{pod}: {metrics}");
    }
    let leases: Api<Lease> = Api::namespaced(client.clone(), &controller_namespace());
    let before = leases.get(LEASE).await.unwrap().spec.unwrap();
    let transitions = before.lease_transitions.unwrap_or(0);

    // 3. The leader's pod goes away. It releases the Lease on shutdown, and the other replica,
    //    or the one replacing it, takes over.
    let pod_api: Api<Pod> = Api::namespaced(client.clone(), &controller_namespace());
    pod_api
        .delete(&leader, &DeleteParams::default())
        .await
        .expect("delete the leader's pod");
    let new_leader = wait_for("a new leader", || async {
        let h = holder(&client).await;
        (h != leader).then_some(h)
    })
    .await;
    let after = leases.get(LEASE).await.unwrap().spec.unwrap();
    assert!(
        after.lease_transitions.unwrap_or(0) > transitions,
        "the take-over is counted: {after:?}"
    );

    // 4. The new leader reconciles: a policy publishes a signed bundle.
    let ns = policy_namespace();
    let name = unique_name("leader");
    let ceilings: Api<ExposureCeiling> = Api::namespaced(client.clone(), &ns);
    ceilings
        .create(
            &PostParams::default(),
            &serde_json::from_value(hardened_ceiling(&name, &ns)).unwrap(),
        )
        .await
        .expect("create ExposureCeiling");
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    policies
        .create(
            &PostParams::default(),
            &serde_json::from_value(hardened_policy(&name, &ns)).unwrap(),
        )
        .await
        .expect("create SandboxPolicy");
    wait_for(&format!("{name} published by {new_leader}"), || {
        let policies = policies.clone();
        let name = name.clone();
        async move {
            let status = policies.get(&name).await.ok()?.status?;
            condition_true(&status.conditions, "Ready", Some("Published")).then_some(())
        }
    })
    .await;
    let _ = policies.delete(&name, &DeleteParams::default()).await;
    let _ = ceilings.delete(&name, &DeleteParams::default()).await;
}
