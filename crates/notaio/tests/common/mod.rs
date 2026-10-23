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

/// The key the deployed controller signs with: OpenBao's transit key (ADR-0004), as
/// dev/openbao/setup.sh printed it, `notaio:v<N> <base64 public key>`, passed by the Makefile.
pub fn openbao_key() -> (String, [u8; 32]) {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let line = std::env::var("NOTAIO_E2E_OPENBAO_PUBLIC_KEY")
        .expect("NOTAIO_E2E_OPENBAO_PUBLIC_KEY is not set: run through make kind-e2e-<suite>");
    let (key_id, b64) = line
        .trim()
        .split_once(' ')
        .expect("NOTAIO_E2E_OPENBAO_PUBLIC_KEY is `<key id> <base64>`");
    let raw: [u8; 32] = STANDARD
        .decode(b64)
        .expect("base64 public key")
        .try_into()
        .expect("an ed25519 public key is 32 bytes");
    (key_id.to_string(), raw)
}

/// What a consumer trusts: OpenBao's public key and nothing else.
pub fn trusted_keys() -> sandboxpolicy_bundle::TrustedKeys {
    let (key_id, public) = openbao_key();
    let mut keys = sandboxpolicy_bundle::TrustedKeys::new();
    keys.insert(&key_id, public).expect("an ed25519 public key");
    keys
}

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

/// The approved ceiling for [`hardened_policy`] (ADR-0007): exactly its exposure, one read-only
/// audience with the K5.1 R0 token, the K7.3 R0 lease. [`package_install_grant`] does not widen it.
/// Same shape as `examples/exposureceiling-agent-default.yaml`.
pub fn hardened_ceiling(policy: &str, namespace: &str) -> Value {
    json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha1",
        "kind": "ExposureCeiling",
        "metadata": { "name": policy, "namespace": namespace },
        "spec": {
            "audiences": [
                { "name": "internal-git", "access": "Read", "maxTtlSeconds": 300 }
            ],
            "egressHosts": [],
            "tools": [],
            "maxLeaseSeconds": 14400,
            "approvals": [
                { "role": "Security", "reference": "https://example.com/review/2" }
            ]
        }
    })
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
