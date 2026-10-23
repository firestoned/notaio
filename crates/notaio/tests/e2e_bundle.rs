//! e2e: a policy goes in, a signed bundle comes out, and a consumer can verify it.
//!
//! Run with `make kind-e2e-bundle`. This is the M1 loop from ROADMAP.md ("a policy, a grant, a
//! bundle published and verified by a small consumer"), driven through the deployed controller.
//! The verification half is exactly what mediatore does: DSSE signature against a trusted key,
//! expiry, and a version floor that refuses a downgrade (ADR-0002).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chrono::Utc;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::api::{DeleteParams, PostParams};
use kube::{Api, ResourceExt};
use sandboxpolicy_api::{ExposureCeiling, GrantPhase, KnobGrant, SandboxPolicy};
use sandboxpolicy_bundle::floor::{accept, FileFloor};
use sandboxpolicy_bundle::{
    verify_bundle, BundleError, BundlePayload, Ed25519Signer, Envelope, Signer, TrustedKeys,
    VerifyOptions,
};

use common::*;

/// The envelope currently published for `policy`, once its version annotation reaches `version`.
async fn published_envelope(cms: &Api<ConfigMap>, policy: &str, version: u64) -> Envelope {
    let name = bundle_object_name(policy);
    let cm = wait_for(
        &format!("ConfigMap {name} at bundle version {version}"),
        || {
            let cms = cms.clone();
            let name = name.clone();
            async move {
                let cm = cms.get_opt(&name).await.ok()??;
                let published = cm.annotations().get(ANN_VERSION)?.parse::<u64>().ok()?;
                (published == version).then_some(cm)
            }
        },
    )
    .await;
    assert_eq!(
        cm.labels()
            .get("app.kubernetes.io/managed-by")
            .map(String::as_str),
        Some(MANAGED_BY)
    );
    let raw = cm
        .data
        .and_then(|d| d.get(BUNDLE_KEY).cloned())
        .unwrap_or_else(|| panic!("ConfigMap {name} has no {BUNDLE_KEY}"));
    serde_json::from_str(&raw).expect("the published bundle is a DSSE envelope")
}

/// Waits for the policy to be Ready with a bundle version strictly above `after`.
async fn wait_published(policies: &Api<SandboxPolicy>, name: &str, after: u64) -> SandboxPolicy {
    wait_for(
        &format!("SandboxPolicy {name} Ready/Published past version {after}"),
        || {
            let policies = policies.clone();
            let name = name.to_string();
            async move {
                let p = policies.get(&name).await.ok()?;
                let status = p.status.as_ref()?;
                let ready = condition_true(&status.conditions, "Ready", Some("Published"));
                let version = status.bundle_version?;
                (ready && version > after).then_some(p)
            }
        },
    )
    .await
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-bundle"]
async fn policy_compiles_to_a_signed_verifiable_bundle_that_never_goes_back() {
    let client = client().await;
    let ns = policy_namespace();
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    let grants: Api<KnobGrant> = Api::namespaced(client.clone(), &ns);
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let keys = trusted_keys();

    let name = unique_name("bundle");
    // The approved ceiling first: without one nothing publishes (ADR-0007).
    let ceilings: Api<ExposureCeiling> = Api::namespaced(client.clone(), &ns);
    let ceiling: ExposureCeiling = serde_json::from_value(hardened_ceiling(&name, &ns))
        .expect("fixture is an ExposureCeiling");
    ceilings
        .create(&PostParams::default(), &ceiling)
        .await
        .expect("create ExposureCeiling");
    let policy: SandboxPolicy =
        serde_json::from_value(hardened_policy(&name, &ns)).expect("fixture is a SandboxPolicy");
    policies
        .create(&PostParams::default(), &policy)
        .await
        .expect("create SandboxPolicy");

    // 1. First bundle: published, and verifiable by a consumer holding only the public key.
    let p1 = wait_published(&policies, &name, 0).await;
    let s1 = p1.status.clone().expect("status");
    let v1 = s1.bundle_version.expect("bundle version");
    // The transition is recorded as an Event on the policy (ADR-0010).
    let ev = wait_event(&client, &ns, &name, "Published").await;
    assert_eq!(ev.type_.as_deref(), Some("Normal"));
    let env1 = published_envelope(&cms, &name, v1).await;
    let payload1: BundlePayload =
        verify_bundle(&env1, &keys, &VerifyOptions::at(Utc::now())).expect("bundle verifies");
    assert_eq!(payload1.meta.version, v1);
    assert_eq!(Some(&payload1.content_digest), s1.bundle_digest.as_ref());
    assert_eq!(payload1.content.policy.name, name);
    assert_eq!(payload1.content.policy.namespace, ns);
    assert_eq!(
        Some(payload1.content.policy.uid.as_str()),
        p1.uid().as_deref()
    );
    assert_eq!(payload1.content.profile.name, "hardened");
    assert_eq!(
        Some(&payload1.content.profile.digest),
        s1.profile_digest.as_ref()
    );

    // A bundle signed by any other key is refused: the consumer's trust is the key, not the
    // ConfigMap it happened to read.
    let (key_id, _) = openbao_key();
    let stranger = Ed25519Signer::from_seed(&key_id, [7u8; 32]);
    let mut wrong = TrustedKeys::new();
    wrong
        .insert(stranger.key_id(), stranger.verifying_key_bytes())
        .expect("key");
    assert!(
        verify_bundle(&env1, &wrong, &VerifyOptions::at(Utc::now())).is_err(),
        "a bundle verified against a key that did not sign it"
    );

    // 2. A one-step grant changes the content, so the version must move forward.
    let grant_name = unique_name("grant");
    let grant: KnobGrant = serde_json::from_value(package_install_grant(&grant_name, &ns, &name))
        .expect("fixture is a KnobGrant");
    grants
        .create(&PostParams::default(), &grant)
        .await
        .expect("create KnobGrant");

    wait_for(&format!("KnobGrant {grant_name} to be Active"), || {
        let grants = grants.clone();
        let grant_name = grant_name.clone();
        async move {
            let g = grants.get(&grant_name).await.ok()?;
            (g.status?.phase == GrantPhase::Active).then_some(())
        }
    })
    .await;

    let p2 = wait_published(&policies, &name, v1).await;
    let s2 = p2.status.clone().expect("status");
    let v2 = s2.bundle_version.expect("bundle version");
    assert!(
        s2.active_grants.contains(&grant_name),
        "policy status does not list the active grant: {:?}",
        s2.active_grants
    );
    let env2 = published_envelope(&cms, &name, v2).await;
    let payload2 =
        verify_bundle(&env2, &keys, &VerifyOptions::at(Utc::now())).expect("bundle verifies");
    assert_ne!(payload2.content_digest, payload1.content_digest);
    // The grant expires in a week, so the bundle must not outlive it.
    let grant_expiry = s2.next_grant_expiry.expect("next grant expiry is reported");
    assert!(payload2.meta.not_after <= grant_expiry);

    // 3. The consumer loop, as mediatore and mediatore-guest run it (ADR-0008): a persisted floor
    //    that survives a restart, so the v1 bundle seen earlier is refused even by a consumer that
    //    has just started again. This is the downgrade the threat model calls T5.4.
    let floor_path =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-floor.json"));
    let _ = std::fs::remove_file(&floor_path);
    {
        let mut floor = FileFloor::open(&floor_path).expect("open floor");
        accept(&env1, &keys, Utc::now(), &mut floor).expect("v1 accepted first");
        accept(&env2, &keys, Utc::now(), &mut floor).expect("v2 accepted after v1");
    }
    let mut restarted = FileFloor::open(&floor_path).expect("reopen floor");
    assert_eq!(
        accept(&env1, &keys, Utc::now(), &mut restarted).unwrap_err(),
        BundleError::Downgrade { got: v1, min: v2 },
        "a restarted consumer at version {v2} accepted version {v1}"
    );
    accept(&env2, &keys, Utc::now(), &mut restarted).expect("the current bundle passes the floor");
    let _ = std::fs::remove_file(&floor_path);

    // Cleanup. The bundle ConfigMap is owned by the policy and goes with it.
    grants
        .delete(&grant_name, &DeleteParams::default())
        .await
        .expect("delete KnobGrant");
    policies
        .delete(&name, &DeleteParams::default())
        .await
        .expect("delete SandboxPolicy");
    ceilings
        .delete(&name, &DeleteParams::default())
        .await
        .expect("delete ExposureCeiling");
}

#[tokio::test]
#[ignore = "needs a kind cluster: make kind-e2e-bundle"]
async fn a_policy_without_an_approved_ceiling_publishes_nothing() {
    // ADR-0007, decision 4: enforced in the cluster, not only at pull request time.
    let client = client().await;
    let ns = policy_namespace();
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let name = unique_name("uncapped");
    let policy: SandboxPolicy =
        serde_json::from_value(hardened_policy(&name, &ns)).expect("fixture is a SandboxPolicy");
    policies
        .create(&PostParams::default(), &policy)
        .await
        .expect("create SandboxPolicy");

    // Wait on the controller's verdict, not on the absence of a ConfigMap, which is true before
    // the controller has looked at all.
    let status = wait_for(
        &format!("SandboxPolicy {name} refused for want of a ceiling"),
        || {
            let policies = policies.clone();
            let name = name.clone();
            async move {
                let status = policies.get(&name).await.ok()?.status?;
                status
                    .conditions
                    .iter()
                    .any(|c| {
                        c.type_ == "Ready" && c.status == "False" && c.reason == "NoApprovedCeiling"
                    })
                    .then_some(status)
            }
        },
    )
    .await;
    assert!(
        status.max_exposure.is_some(),
        "the exposure to approve is reported"
    );
    let ev = wait_event(&client, &ns, &name, "NoApprovedCeiling").await;
    assert_eq!(ev.type_.as_deref(), Some("Warning"));
    assert!(
        cms.get_opt(&bundle_object_name(&name))
            .await
            .expect("get")
            .is_none(),
        "a bundle was published without an approved ceiling"
    );
    policies
        .delete(&name, &DeleteParams::default())
        .await
        .expect("delete SandboxPolicy");
}
