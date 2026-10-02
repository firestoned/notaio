//! Shared helpers for the kind e2e suites (`make kind-e2e`, `.github/workflows/e2e.yaml`).
//!
//! Every suite is `#[ignore]`d, so running one is already an explicit request to talk to a
//! cluster: an unreachable cluster or a missing fixture is a failure, never a silent skip
//! (`.claude/rules/testing.md`). The Makefile supplies `KUBECONFIG` and the namespaces.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::future::Future;
use std::time::Duration;

use chrono::Utc;
use kube::Client;
use serde_json::{json, Value};

/// How long any single wait may take before the suite fails. A cold controller on a fresh kind
/// cluster reconciles in seconds; this leaves room for a slow CI runner, not for a missing feature.
pub const CONVERGE_TIMEOUT: Duration = Duration::from_secs(120);

/// Poll interval for [`wait_for`].
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The published contract a consumer relies on, restated here on purpose. These values live in
/// `crates/notaio/src/reconcile.rs`; if one changes there, this suite must fail until the change
/// is made deliberately in both places.
pub const BUNDLE_KEY: &str = "bundle.dsse.json";
pub const ANN_VERSION: &str = "sandbox.firestoned.io/bundle-version";
pub const ANN_DIGEST: &str = "sandbox.firestoned.io/bundle-digest";
pub const MANAGED_BY: &str = "notaio";

/// Name of the development signing key Secret and the key id the Deployment signs with
/// (`deploy/controller/20-deployment.yaml`).
pub const SIGNING_SECRET: &str = "notaio-dev-signing-key";
pub const SIGNING_KEY_ID: &str = "dev";

/// Name of the published bundle ConfigMap for a policy.
pub fn bundle_object_name(policy: &str) -> String {
    format!("{policy}-bundle")
}

/// A client for the cluster in `KUBECONFIG`. Panics, and so fails the test, when there is none.
pub async fn client() -> Client {
    Client::try_default()
        .await
        .expect("no usable cluster: run through `make kind-e2e-<suite>`, which sets KUBECONFIG")
}

/// Namespace the controller is installed in.
pub fn controller_namespace() -> String {
    required_env("NOTAIO_E2E_NAMESPACE")
}

/// Namespace that holds the SandboxPolicy objects under test.
pub fn policy_namespace() -> String {
    required_env("NOTAIO_E2E_POLICY_NAMESPACE")
}

/// The controller's own identity, as the API server sees it.
pub fn controller_user() -> String {
    format!("system:serviceaccount:{}:notaio", controller_namespace())
}

fn required_env(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} is not set: run through `make kind-e2e-<suite>`"))
}

/// A name no earlier run on the same cluster has used.
pub fn unique_name(prefix: &str) -> String {
    format!("e2e-{prefix}-{}", Utc::now().timestamp_millis())
}

/// Polls `check` until it returns `Some`, or panics naming `what` after [`CONVERGE_TIMEOUT`].
///
/// Wait on the property that is actually new. A condition that stale state can already satisfy
/// returns instantly and proves nothing (`.claude/rules/testing.md`).
pub async fn wait_for<T, F, Fut>(what: &str, mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let deadline = tokio::time::Instant::now() + CONVERGE_TIMEOUT;
    loop {
        if let Some(v) = check().await {
            return v;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out after {CONVERGE_TIMEOUT:?} waiting for {what}"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// A SandboxPolicy on the `hardened` profile that passes every lint rule. Same shape as
/// `examples/sandboxpolicy-hardened.yaml`.
pub fn hardened_policy(name: &str, namespace: &str) -> Value {
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha1",
        "kind": "SandboxPolicy",
        "metadata": { "name": name, "namespace": namespace },
        "spec": {
            "profileRef": "hardened",
            "subjects": { "groups": ["00000000-0000-0000-0000-000000000000"] },
            "audiences": [
                { "name": "internal-git", "access": "Read", "scopes": ["repo:read"] }
            ],
            "egress": [],
            "tools": []
        }
    })
}

/// A one-step KnobGrant (K2.3 R0 to R1) for `policy`, expiring a week from now. Same shape as
/// `examples/knobgrant-package-install.yaml`, but never expired: a fixed date would turn this
/// suite red on that day.
pub fn package_install_grant(name: &str, namespace: &str, policy: &str) -> Value {
    let expires = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha1",
        "kind": "KnobGrant",
        "metadata": { "name": name, "namespace": namespace },
        "spec": {
            "policyRef": policy,
            "knob": "K2.3",
            "step": "R1",
            "owner": "team-e2e",
            "reason": "e2e: exercise a one-step relaxation",
            "expiresAt": expires,
            "approvals": [
                { "role": "PlatformOwner", "reference": "https://example.com/review/1" }
            ]
        }
    })
}

/// True when `conditions` holds `type_` with status `True`, and, if given, `reason`.
pub fn condition_true(
    conditions: &[sandboxpolicy_api::Condition],
    type_: &str,
    reason: Option<&str>,
) -> bool {
    conditions
        .iter()
        .any(|c| c.type_ == type_ && c.status == "True" && reason.is_none_or(|r| c.reason == r))
}
