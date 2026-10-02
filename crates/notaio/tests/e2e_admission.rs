//! e2e: `deploy/admission/gitops-only.yaml` refuses direct edits to policy objects, lets the GitOps
//! reconciler through, and does not block the controller's status writes.
//!
//! Run with `make kind-e2e-admission`, which applies the admission policy first and always removes
//! it afterwards. Threat model T8.2 and scenario S6: a policy changes only through a reviewed
//! change. The manifest's own header said "Not yet tested on a cluster"; this is that test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding};
use kube::api::{DeleteParams, PostParams};
use kube::{Api, Client, Config};
use sandboxpolicy_api::SandboxPolicy;
use serde_json::json;

use common::*;

/// The only identity `gitops-only.yaml` admits.
const GITOPS_USER: &str = "system:serviceaccount:flux-system:kustomize-controller";

/// RBAC so the impersonated GitOps identity may write policy objects at all. Admission runs after
/// authorization, so without this every request would be a 403 from RBAC, proving nothing.
const GITOPS_RBAC: &str = "notaio-e2e-gitops";

/// Text of the policy's denial message (`deploy/admission/gitops-only.yaml`).
const DENIAL: &str = "may only be changed by the GitOps reconciler";

async fn as_gitops() -> Client {
    let mut config = Config::infer().await.expect("kubeconfig");
    config.auth_info.impersonate = Some(GITOPS_USER.to_string());
    Client::try_from(config).expect("impersonating client")
}

fn policy(name: &str, ns: &str) -> SandboxPolicy {
    serde_json::from_value(hardened_policy(name, ns)).expect("fixture is a SandboxPolicy")
}

/// Waits until the admission policy is enforcing. A ValidatingAdmissionPolicy takes a moment to
/// reach the API server's cache after it is created, so probe with a server-side dry run: it goes
/// through admission, but persists nothing, so a probe that lands early leaves no object behind.
async fn wait_enforcing(admin: &Client, ns: &str) {
    let policies: Api<SandboxPolicy> = Api::namespaced(admin.clone(), ns);
    let probe = policy(&unique_name("probe"), ns);
    let dry_run = PostParams {
        dry_run: true,
        ..Default::default()
    };
    wait_for(
        "the gitops-only admission policy to deny a direct create",
        || {
            let policies = policies.clone();
            let probe = probe.clone();
            let dry_run = dry_run.clone();
            async move {
                match policies.create(&dry_run, &probe).await {
                    Err(e) if e.to_string().contains(DENIAL) => Some(()),
                    _ => None,
                }
            }
        },
    )
    .await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-admission"]
async fn direct_edits_are_denied_even_for_cluster_admin() {
    let admin = client().await;
    let ns = policy_namespace();
    wait_enforcing(&admin, &ns).await;

    let policies: Api<SandboxPolicy> = Api::namespaced(admin.clone(), &ns);
    let err = policies
        .create(&PostParams::default(), &policy(&unique_name("direct"), &ns))
        .await
        .expect_err("cluster-admin created a SandboxPolicy directly");
    assert!(
        err.to_string().contains(DENIAL),
        "denied, but not by the gitops-only policy: {err}"
    );
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-admission"]
async fn gitops_reconciler_is_admitted_and_controller_still_writes_status() {
    let admin = client().await;
    let ns = policy_namespace();

    let roles: Api<ClusterRole> = Api::all(admin.clone());
    let bindings: Api<ClusterRoleBinding> = Api::all(admin.clone());
    let role: ClusterRole = serde_json::from_value(json!({
        "metadata": { "name": GITOPS_RBAC },
        "rules": [{
            "apiGroups": ["sandbox.firestoned.io"],
            "resources": ["sandboxprofiles", "sandboxpolicies", "knobgrants"],
            "verbs": ["get", "list", "watch", "create", "update", "patch", "delete"]
        }]
    }))
    .expect("ClusterRole");
    let binding: ClusterRoleBinding = serde_json::from_value(json!({
        "metadata": { "name": GITOPS_RBAC },
        "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": GITOPS_RBAC },
        "subjects": [{ "kind": "ServiceAccount", "name": "kustomize-controller", "namespace": "flux-system" }]
    }))
    .expect("ClusterRoleBinding");
    // Tolerate leftovers from an interrupted earlier run.
    let _ = roles.delete(GITOPS_RBAC, &DeleteParams::default()).await;
    let _ = bindings.delete(GITOPS_RBAC, &DeleteParams::default()).await;
    roles
        .create(&PostParams::default(), &role)
        .await
        .expect("create ClusterRole");
    bindings
        .create(&PostParams::default(), &binding)
        .await
        .expect("create ClusterRoleBinding");

    // Only meaningful while the policy is enforcing, or "admitted" would prove nothing.
    wait_enforcing(&admin, &ns).await;

    let name = unique_name("gitops");
    let gitops: Api<SandboxPolicy> = Api::namespaced(as_gitops().await, &ns);
    wait_for("the GitOps identity to create a SandboxPolicy", || {
        let gitops = gitops.clone();
        let p = policy(&name, &ns);
        async move {
            gitops
                .create(&PostParams::default(), &p)
                .await
                .ok()
                .map(|_| ())
        }
    })
    .await;

    // The policy matches the main resource only, so the controller's /status writes pass.
    let policies: Api<SandboxPolicy> = Api::namespaced(admin.clone(), &ns);
    wait_for(
        &format!("the controller to publish {name} under admission"),
        || {
            let policies = policies.clone();
            let name = name.clone();
            async move {
                let status = policies.get(&name).await.ok()?.status?;
                condition_true(&status.conditions, "Ready", Some("Published")).then_some(())
            }
        },
    )
    .await;

    // Deleting is an edit too: refused for the admin, allowed for GitOps.
    let err = policies
        .delete(&name, &DeleteParams::default())
        .await
        .expect_err("cluster-admin deleted a SandboxPolicy directly");
    assert!(err.to_string().contains(DENIAL), "unexpected denial: {err}");
    gitops
        .delete(&name, &DeleteParams::default())
        .await
        .expect("GitOps identity deletes its SandboxPolicy");

    bindings
        .delete(GITOPS_RBAC, &DeleteParams::default())
        .await
        .expect("delete ClusterRoleBinding");
    roles
        .delete(GITOPS_RBAC, &DeleteParams::default())
        .await
        .expect("delete ClusterRole");
}
