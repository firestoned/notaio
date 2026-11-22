// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! e2e: `deploy/admission/gitops-only.yaml` refuses direct edits to policy objects, lets the GitOps
//! reconciler through, and admits status writes from the controller and from no one else
//! (ADR-0021).
//!
//! Run with `make kind-e2e-admission`, which applies the admission policy first and always removes
//! it afterwards. Threat model T8.2 and scenario S6: a policy changes only through a reviewed
//! change. The manifest's own header said "Not yet tested on a cluster"; this is that test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding};
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::{Api, Client, Config};
use sandboxpolicy_api::{ExposureCeiling, SandboxAudience, SandboxPolicy, SandboxProfile};
use serde_json::json;

use common::*;

/// The only identity `gitops-only.yaml` admits.
const GITOPS_USER: &str = "system:serviceaccount:flux-system:kustomize-controller";

/// RBAC so the impersonated GitOps identity may write policy objects at all. Admission runs after
/// authorization, so without this every request would be a 403 from RBAC, proving nothing.
const GITOPS_RBAC: &str = "notaio-e2e-gitops";

/// Text of the policy's denial message (`deploy/admission/gitops-only.yaml`).
const DENIAL: &str = "may only be changed by the GitOps reconciler";

/// Text of the status rule's denial message (`deploy/admission/gitops-only.yaml`, ADR-0021).
const STATUS_DENIAL: &str = "status is written only by notaio";

/// A built-in profile `make kind-deploy` installs: an object whose status a probe can target.
const PROBE_PROFILE: &str = "hardened";

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

/// Waits until the status rule (ADR-0021) is enforcing, with a server-side dry run of a status
/// patch on a built-in profile: it goes through admission and persists nothing.
async fn wait_status_enforcing(admin: &Client) {
    let profiles: Api<SandboxProfile> = Api::all(admin.clone());
    let dry_run = PatchParams {
        dry_run: true,
        ..Default::default()
    };
    wait_for(
        "the status admission policy to deny a status patch from cluster-admin",
        || {
            let profiles = profiles.clone();
            let dry_run = dry_run.clone();
            async move {
                let patch = Patch::Merge(json!({ "status": { "digest": "sha256:forged" } }));
                match profiles.patch_status(PROBE_PROFILE, &dry_run, &patch).await {
                    Err(e) if e.to_string().contains(STATUS_DENIAL) => Some(()),
                    _ => None,
                }
            }
        },
    )
    .await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-admission"]
async fn status_writes_are_denied_even_for_cluster_admin() {
    let admin = client().await;
    wait_status_enforcing(&admin).await;

    // A real write, not a dry run: refused, and nothing changed. cluster-admin has every RBAC
    // permission, so the refusal can only be admission's (ADR-0021, decision 1).
    let profiles: Api<SandboxProfile> = Api::all(admin.clone());
    let before = profiles.get(PROBE_PROFILE).await.expect("built-in profile");
    let err = profiles
        .patch_status(
            PROBE_PROFILE,
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": { "digest": "sha256:forged" } })),
        )
        .await
        .expect_err("cluster-admin patched a SandboxProfile's status");
    assert!(
        err.to_string().contains(STATUS_DENIAL),
        "denied, but not by the status policy: {err}"
    );
    let after = profiles.get(PROBE_PROFILE).await.expect("built-in profile");
    assert_eq!(
        before.status.and_then(|s| s.digest),
        after.status.and_then(|s| s.digest),
        "a refused status patch changed the profile's status"
    );
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

    // Ceilings too (ADR-0007): an approval nobody reviewed is no approval.
    let ceilings: Api<ExposureCeiling> = Api::namespaced(admin.clone(), &ns);
    let ceiling: ExposureCeiling =
        serde_json::from_value(hardened_ceiling(&unique_name("direct"), &ns)).expect("fixture");
    let err = ceilings
        .create(&PostParams::default(), &ceiling)
        .await
        .expect_err("cluster-admin created an ExposureCeiling directly");
    assert!(err.to_string().contains(DENIAL), "unexpected denial: {err}");

    // Audience definitions too (ADR-0020, decision 12): a definition nobody reviewed could point a
    // credential at an endpoint of its author's choosing.
    let audiences: Api<SandboxAudience> = Api::namespaced(admin.clone(), &ns);
    let audience: SandboxAudience = serde_json::from_value(json!({
        "apiVersion": "sandbox.firestoned.io/v1alpha2",
        "kind": "SandboxAudience",
        "metadata": { "name": unique_name("direct"), "namespace": ns },
        "spec": {
            "credential": { "backend": "none", "placement": "gateway" },
            "endpoints": ["git.foo.io:443"],
            "maxTtlSeconds": 300
        }
    }))
    .expect("fixture");
    let err = audiences
        .create(&PostParams::default(), &audience)
        .await
        .expect_err("cluster-admin created a SandboxAudience directly");
    assert!(err.to_string().contains(DENIAL), "unexpected denial: {err}");
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
            "resources": ["sandboxprofiles", "sandboxpolicies", "knobgrants", "exposureceilings", "sandboxaudiences"],
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

    // Only meaningful while both rules are enforcing, or "admitted" would prove nothing.
    wait_enforcing(&admin, &ns).await;
    wait_status_enforcing(&admin).await;

    let name = unique_name("gitops");
    let gitops_client = as_gitops().await;
    let gitops_ceilings: Api<ExposureCeiling> = Api::namespaced(gitops_client.clone(), &ns);
    let ceiling: ExposureCeiling =
        serde_json::from_value(hardened_ceiling(&name, &ns)).expect("fixture");
    wait_for("the GitOps identity to create an ExposureCeiling", || {
        let gitops_ceilings = gitops_ceilings.clone();
        let c = ceiling.clone();
        async move {
            gitops_ceilings
                .create(&PostParams::default(), &c)
                .await
                .ok()
                .map(|_| ())
        }
    })
    .await;
    let gitops: Api<SandboxPolicy> = Api::namespaced(gitops_client, &ns);
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

    // The spec rule matches the main resource only, and the status rule admits the controller's
    // own identity (ADR-0021), so its /status writes pass.
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

    // Another identity is refused the same status write, even with every RBAC permission: a
    // forged version would make notaio publish below what consumers hold (ADR-0021).
    let err = policies
        .patch_status(
            &name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": { "bundleVersion": 1 } })),
        )
        .await
        .expect_err("cluster-admin patched a SandboxPolicy's status");
    assert!(
        err.to_string().contains(STATUS_DENIAL),
        "unexpected denial: {err}"
    );

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
    gitops_ceilings
        .delete(&name, &DeleteParams::default())
        .await
        .expect("GitOps identity deletes its ExposureCeiling");

    bindings
        .delete(GITOPS_RBAC, &DeleteParams::default())
        .await
        .expect("delete ClusterRoleBinding");
    roles
        .delete(GITOPS_RBAC, &DeleteParams::default())
        .await
        .expect("delete ClusterRole");
}
