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

use common::{client, condition_true, controller_namespace, wait_for};

/// Names of the built-in profiles, read from the generated manifest the Makefile applied.
fn builtin_profile_names() -> Vec<String> {
    let raw = include_str!("../../../deploy/profiles/builtin.yaml");
    serde_yaml_ng::Deserializer::from_str(raw)
        .map(|doc| {
            let v: Value = serde::Deserialize::deserialize(doc).expect("builtin.yaml parses");
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
