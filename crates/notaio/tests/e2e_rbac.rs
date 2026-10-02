//! e2e: the controller's deployed RBAC is read plus status writes, and nothing more.
//!
//! Run with `make kind-e2e-rbac`. CLAUDE.md: "Do not make the controller able to create, edit or
//! delete policy objects. Its RBAC is read plus status writes by design." This suite asks the API
//! server itself, through SubjectAccessReview, what the ServiceAccount from
//! `deploy/controller/10-rbac.yaml` may do. A widened ClusterRole fails here even when every unit
//! test passes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SubjectAccessReview, SubjectAccessReviewSpec,
};
use kube::api::PostParams;
use kube::{Api, Client};

use common::{client, controller_namespace, controller_user, policy_namespace};

const GROUP: &str = "sandbox.firestoned.io";
const RBAC: &str = "rbac.authorization.k8s.io";
const POLICY_KINDS: [&str; 3] = ["sandboxprofiles", "sandboxpolicies", "knobgrants"];

/// Asks the API server whether the controller may `verb` on `resource` (optionally a
/// `subresource`) in `namespace` (None for cluster scope).
async fn may(
    client: &Client,
    verb: &str,
    group: &str,
    resource: &str,
    subresource: Option<&str>,
    namespace: Option<&str>,
) -> bool {
    let ns = controller_namespace();
    let review = SubjectAccessReview {
        spec: SubjectAccessReviewSpec {
            user: Some(controller_user()),
            // The groups a ServiceAccount token really carries, so a grant to any of them counts.
            groups: Some(vec![
                "system:serviceaccounts".to_string(),
                format!("system:serviceaccounts:{ns}"),
                "system:authenticated".to_string(),
            ]),
            resource_attributes: Some(ResourceAttributes {
                verb: Some(verb.to_string()),
                group: Some(group.to_string()),
                resource: Some(resource.to_string()),
                subresource: subresource.map(str::to_string),
                namespace: namespace.map(str::to_string),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let reviews: Api<SubjectAccessReview> = Api::all(client.clone());
    reviews
        .create(&PostParams::default(), &review)
        .await
        .expect("SubjectAccessReview")
        .status
        .expect("SubjectAccessReview status")
        .allowed
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-rbac"]
async fn controller_can_read_policy_objects_and_write_their_status() {
    let client = client().await;
    let policy_ns = policy_namespace();
    for kind in POLICY_KINDS {
        for verb in ["get", "list", "watch"] {
            assert!(
                may(&client, verb, GROUP, kind, None, None).await,
                "controller cannot {verb} {kind}"
            );
        }
        assert!(
            may(
                &client,
                "patch",
                GROUP,
                kind,
                Some("status"),
                Some(&policy_ns)
            )
            .await,
            "controller cannot patch {kind}/status"
        );
    }
    for verb in ["get", "create", "patch"] {
        assert!(
            may(&client, verb, "", "configmaps", None, Some(&policy_ns)).await,
            "controller cannot {verb} configmaps in {policy_ns}, so it cannot publish bundles"
        );
    }
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-rbac"]
async fn controller_cannot_create_edit_or_delete_policy_objects() {
    let client = client().await;
    let policy_ns = policy_namespace();
    for kind in POLICY_KINDS {
        for verb in ["create", "update", "patch", "delete", "deletecollection"] {
            for ns in [None, Some(policy_ns.as_str())] {
                assert!(
                    !may(&client, verb, GROUP, kind, None, ns).await,
                    "controller may {verb} {kind} (namespace {ns:?}): policy objects arrive only \
                     through review and GitOps (threat model T8.2)"
                );
            }
        }
    }
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-rbac"]
async fn controller_has_no_reach_beyond_its_bundles() {
    let client = client().await;
    let ns = controller_namespace();
    let policy_ns = policy_namespace();

    // The signing key is mounted, never read through the API. No Secret access anywhere.
    for verb in ["get", "list", "watch"] {
        for scope in [None, Some(ns.as_str()), Some(policy_ns.as_str())] {
            assert!(
                !may(&client, verb, "", "secrets", None, scope).await,
                "controller may {verb} secrets (namespace {scope:?})"
            );
        }
    }

    // Bundle writes are per namespace, never cluster wide, and never a delete.
    assert!(!may(&client, "create", "", "configmaps", None, Some("default")).await);
    assert!(!may(&client, "create", "", "configmaps", None, None).await);
    assert!(!may(&client, "delete", "", "configmaps", None, Some(&policy_ns)).await);

    // No workloads, no RBAC of its own making.
    assert!(!may(&client, "create", "", "pods", None, Some(&policy_ns)).await);
    assert!(!may(&client, "create", RBAC, "clusterroles", None, None).await);
    assert!(!may(&client, "bind", RBAC, "clusterroles", None, None).await);
    assert!(!may(&client, "escalate", RBAC, "clusterroles", None, None).await);
}
