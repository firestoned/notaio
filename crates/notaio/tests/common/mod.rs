// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
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
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
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
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
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
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
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

/// The `events.k8s.io` Event notaio recorded about `regarding` in `namespace` with `reason`, once
/// there is one (ADR-0010, decision 5).
pub async fn wait_event(
    client: &Client,
    namespace: &str,
    regarding: &str,
    reason: &str,
) -> k8s_openapi::api::events::v1::Event {
    let events: kube::Api<k8s_openapi::api::events::v1::Event> =
        kube::Api::namespaced(client.clone(), namespace);
    wait_for(
        &format!("an Event {reason} on {namespace}/{regarding}"),
        || {
            let events = events.clone();
            async move {
                let list = events.list(&Default::default()).await.ok()?;
                list.items.into_iter().find(|e| {
                    e.reason.as_deref() == Some(reason)
                        && e.reporting_controller.as_deref() == Some("notaio")
                        && e.regarding.as_ref().and_then(|r| r.name.as_deref()) == Some(regarding)
                })
            }
        },
    )
    .await
}

/// The names of the controller's running pods that are not being deleted.
pub async fn running_controller_pods(client: &Client) -> Vec<String> {
    use k8s_openapi::api::core::v1::Pod;
    let pods: kube::Api<Pod> = kube::Api::namespaced(client.clone(), &controller_namespace());
    let Ok(list) = pods
        .list(&kube::api::ListParams::default().labels("app.kubernetes.io/name=notaio"))
        .await
    else {
        return Vec::new();
    };
    list.items
        .into_iter()
        .filter_map(|p| {
            let running = p.status.as_ref()?.phase.as_deref() == Some("Running");
            let deleting = p.metadata.deletion_timestamp.is_some();
            (running && !deleting)
                .then(|| p.metadata.name.clone())
                .flatten()
        })
        .collect()
}

/// `GET path` on one running controller pod. See [`controller_get_from`].
pub async fn controller_get(client: &Client, path: &str) -> (u16, String) {
    let pod = wait_for("a running notaio pod", || async {
        running_controller_pods(client).await.into_iter().next()
    })
    .await;
    controller_get_from(client, &pod, path).await
}

/// `GET path` on `pod`'s health and metrics port, through a port-forward: the kubelet connects
/// inside the pod, so this reads what the process serves whatever the NetworkPolicy admits.
/// Returns the status code and the body.
pub async fn controller_get_from(client: &Client, pod: &str, path: &str) -> (u16, String) {
    use k8s_openapi::api::core::v1::Pod;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let pods: kube::Api<Pod> = kube::Api::namespaced(client.clone(), &controller_namespace());
    let mut pf = pods.portforward(pod, &[8080]).await.expect("port-forward");
    let mut stream = pf.take_stream(8080).expect("the forwarded stream");
    let request = format!("GET {path} HTTP/1.1\r\nhost: notaio\r\nconnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read");
    drop(stream);
    let _ = pf.join().await;
    let text = String::from_utf8(raw).expect("utf-8 response");
    let (head, body) = text.split_once("\r\n\r\n").expect("an HTTP response");
    let code = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .expect("a status code");
    (code, body.to_string())
}
