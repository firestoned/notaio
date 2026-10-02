//! e2e: a policy goes in, a signed bundle comes out, and a consumer can verify it.
//!
//! Run with `make kind-e2e-bundle`. This is the M1 loop from ROADMAP.md ("a policy, a grant, a
//! bundle published and verified by a small consumer"), driven through the deployed controller.
//! The verification half is exactly what mediatore does: DSSE signature against a trusted key,
//! expiry, and a version floor that refuses a downgrade (ADR-0002).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chrono::Utc;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::api::{DeleteParams, PostParams};
use kube::{Api, Client, ResourceExt};
use sandboxpolicy_api::{GrantPhase, KnobGrant, SandboxPolicy};
use sandboxpolicy_bundle::{
    verify_bundle, BundlePayload, Ed25519Signer, Envelope, Signer, TrustedKeys, VerifyOptions,
};

use common::*;

/// The public half of the development key the controller signs with, read back from the Secret
/// the Makefile created. A consumer is configured with exactly this.
async fn trusted_keys(client: &Client) -> TrustedKeys {
    let secrets: Api<Secret> = Api::namespaced(client.clone(), &controller_namespace());
    let secret = secrets
        .get(SIGNING_SECRET)
        .await
        .expect("the development signing key Secret exists");
    let seed = secret
        .data
        .and_then(|d| d.get("seed").cloned())
        .expect("the signing Secret has a `seed` key");
    let seed = String::from_utf8(seed.0).expect("seed is base64 text");
    let signer = Ed25519Signer::from_seed_b64(SIGNING_KEY_ID, &seed).expect("seed is valid");
    let mut keys = TrustedKeys::new();
    keys.insert(signer.key_id(), signer.verifying_key_bytes())
        .expect("a 32 byte ed25519 public key");
    keys
}

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
    let keys = trusted_keys(&client).await;

    let name = unique_name("bundle");
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
    let stranger = Ed25519Signer::from_seed(SIGNING_KEY_ID, [7u8; 32]);
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

    // 3. A consumer that has accepted v2 refuses the v1 bundle it saw earlier: the downgrade the
    //    threat model calls T5.4.
    let mut floor = VerifyOptions::at(Utc::now());
    floor.min_version = Some(v2);
    assert!(
        verify_bundle(&env1, &keys, &floor).is_err(),
        "a consumer at version {v2} accepted version {v1}"
    );
    verify_bundle(&env2, &keys, &floor).expect("the current bundle passes the floor");

    // Cleanup. The bundle ConfigMap is owned by the policy and goes with it.
    grants
        .delete(&grant_name, &DeleteParams::default())
        .await
        .expect("delete KnobGrant");
    policies
        .delete(&name, &DeleteParams::default())
        .await
        .expect("delete SandboxPolicy");
}
