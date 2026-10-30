// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: install from `deploy/` and check the controller comes up and judges the built-in profiles.
//!
//! Run with `make kind-e2e-install`. The Makefile has already applied the CRDs, the controller
//! manifests (real image, RBAC, NetworkPolicy, restricted PSA) and `deploy/profiles/`, and has
//! validated `examples/` with a server-side dry run.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use k8s_openapi::api::apps::v1::Deployment;
use kube::Api;
use sandboxpolicy_api::SandboxProfile;
use serde_json::Value;

use common::{client, condition_true, controller_get, controller_namespace, wait_for};

/// Names of the built-in profiles, read from the generated manifest the Makefile applied.
fn builtin_profile_names() -> Vec<String> {
    let raw = include_str!("../../../deploy/profiles/builtin.yaml");
    let docs: Vec<Value> = serde_saphyr::from_multiple(raw).expect("builtin.yaml parses");
    docs.iter()
        .map(|v| {
            v["metadata"]["name"]
                .as_str()
                .expect("every built-in profile has a name")
                .to_string()
        })
        .collect()
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-install"]
async fn controller_deployment_is_available() {
    let deployments: Api<Deployment> = Api::namespaced(client().await, &controller_namespace());
    wait_for("deployment/notaio to report an available replica", || {
        let deployments = deployments.clone();
        async move {
            let d = deployments.get("notaio").await.ok()?;
            let available = d.status?.available_replicas.unwrap_or(0);
            (available >= 1).then_some(())
        }
    })
    .await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-install"]
async fn every_builtin_profile_is_judged_valid() {
    let names = builtin_profile_names();
    assert!(
        !names.is_empty(),
        "deploy/profiles/builtin.yaml holds no profiles"
    );

    let profiles: Api<SandboxProfile> = Api::all(client().await);
    for name in names {
        let status = wait_for(&format!("SandboxProfile {name} to be Valid"), || {
            let profiles = profiles.clone();
            let name = name.clone();
            async move {
                let status = profiles.get(&name).await.ok()?.status?;
                condition_true(&status.conditions, "Valid", None).then_some(status)
            }
        })
        .await;
        // A digest is only set for a valid profile, and a policy's bundle pins it.
        let digest = status.digest.expect("a Valid profile carries a digest");
        assert!(
            digest.starts_with("sha256:"),
            "profile {name} digest {digest:?} is not sha256:<hex>"
        );
    }
}

/// The value of the sample `name` (labels included) in a Prometheus text exposition.
fn sample(exposition: &str, name: &str) -> Option<f64> {
    exposition
        .lines()
        .find_map(|l| l.strip_prefix(name)?.strip_prefix(' ')?.parse().ok())
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-install"]
async fn controller_serves_health_and_metrics_and_nothing_failed_open() {
    // ADR-0010. The probes in the Deployment already gate readiness; this reads the answers.
    let client = client().await;
    let (code, _) = controller_get(&client, "/healthz").await;
    assert_eq!(code, 200);
    let (code, _) = controller_get(&client, "/readyz").await;
    assert_eq!(
        code, 200,
        "deployed and available, so its watches have listed"
    );
    let (code, _) = controller_get(&client, "/debug").await;
    assert_eq!(code, 404, "only the three paths exist");

    let (code, metrics) = controller_get(&client, "/metrics").await;
    assert_eq!(code, 200);
    assert_eq!(
        sample(&metrics, "notaio_relaxations_past_expiry"),
        Some(0.0),
        "{metrics}"
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
            "fail-open check {check}: {metrics}"
        );
    }
    // The built-in profiles are in the cache the gauges read.
    assert_eq!(
        sample(&metrics, "notaio_lint_failures{kind=\"SandboxProfile\"}"),
        Some(0.0),
        "{metrics}"
    );
}
