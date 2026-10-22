//! e2e: the production signer, an OpenBao transit key (ADR-0004), against the deployed controller.
//!
//! Run with `make kind-e2e-openbao` (kind: cert-manager and dev/openbao installed for the run) or
//! `make openbao-e2e` (a test cluster that already has dev/openbao). The Makefile has switched the
//! controller to OpenBao with dev/openbao/use-signer.sh and passes OpenBao's public key as
//! `NOTAIO_E2E_OPENBAO_PUBLIC_KEY` (`notaio:v<N> <base64>`, from setup.sh).
//!
//! One test, in order, because its second half takes OpenBao away: the Makefile unseals it again
//! afterwards.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{ConfigMap, Pod};
use kube::api::{DeleteParams, ListParams, Patch, PatchParams, PostParams};
use kube::{Api, Client};
use sandboxpolicy_api::{ExposureCeiling, SandboxPolicy};
use sandboxpolicy_bundle::{verify_bundle, Envelope, TrustedKeys, VerifyOptions};
use serde_json::json;

use common::*;

const OPENBAO_NAMESPACE: &str = "openbao";

/// The key consumers trust, as setup.sh printed it: `notaio:v<N> <base64 public key>`.
fn openbao_key() -> (String, [u8; 32]) {
    let line = std::env::var("NOTAIO_E2E_OPENBAO_PUBLIC_KEY")
        .expect("NOTAIO_E2E_OPENBAO_PUBLIC_KEY is not set: run through make kind-e2e-openbao");
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

async fn create_capped_policy(client: &Client, ns: &str, name: &str) {
    let ceilings: Api<ExposureCeiling> = Api::namespaced(client.clone(), ns);
    let ceiling: ExposureCeiling =
        serde_json::from_value(hardened_ceiling(name, ns)).expect("fixture");
    ceilings
        .create(&PostParams::default(), &ceiling)
        .await
        .expect("create ExposureCeiling");
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), ns);
    let policy: SandboxPolicy = serde_json::from_value(hardened_policy(name, ns)).expect("fixture");
    policies
        .create(&PostParams::default(), &policy)
        .await
        .expect("create SandboxPolicy");
}

async fn delete_capped_policy(client: &Client, ns: &str, name: &str) {
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), ns);
    let ceilings: Api<ExposureCeiling> = Api::namespaced(client.clone(), ns);
    let _ = policies.delete(name, &DeleteParams::default()).await;
    let _ = ceilings.delete(name, &DeleteParams::default()).await;
}

/// The policy's Ready condition, once its reason is `reason`.
async fn wait_ready_reason(policies: &Api<SandboxPolicy>, name: &str, reason: &str) -> String {
    wait_for(
        &format!("SandboxPolicy {name} Ready reason {reason}"),
        || {
            let policies = policies.clone();
            let name = name.to_string();
            let reason = reason.to_string();
            async move {
                let status = policies.get(&name).await.ok()?.status?;
                status
                    .conditions
                    .into_iter()
                    .find(|c| c.type_ == "Ready" && c.reason == reason)
                    .map(|c| c.message)
            }
        },
    )
    .await
}

#[tokio::test]
#[ignore = "needs a cluster with dev/openbao: make kind-e2e-openbao"]
async fn signs_through_openbao_and_fails_closed_without_it() {
    let client = client().await;
    let ns = policy_namespace();
    let (key_id, public) = openbao_key();

    // 0. The deployed controller is configured for OpenBao and holds no development seed.
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), &controller_namespace());
    let d = deployments.get("notaio").await.expect("deployment/notaio");
    let env: Vec<String> = d.spec.unwrap().template.spec.unwrap().containers[0]
        .env
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(env.contains(&"NOTAIO_OPENBAO_ADDR".to_string()), "{env:?}");
    assert!(
        !env.contains(&"NOTAIO_SIGNING_KEY_FILE".to_string()),
        "the development signer is still configured: {env:?}"
    );

    // 1. A policy publishes a bundle signed by the transit key, which verifies against OpenBao's
    //    public key and against nothing else.
    let signed = unique_name("openbao");
    create_capped_policy(&client, &ns, &signed).await;
    let policies: Api<SandboxPolicy> = Api::namespaced(client.clone(), &ns);
    wait_ready_reason(&policies, &signed, "Published").await;
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let cm = cms
        .get(&bundle_object_name(&signed))
        .await
        .expect("bundle ConfigMap");
    let env: Envelope =
        serde_json::from_str(&cm.data.unwrap()[BUNDLE_KEY]).expect("a DSSE envelope");
    assert_eq!(env.signatures.len(), 1);
    assert_eq!(env.signatures[0].keyid, key_id, "signed by OpenBao's key");
    assert!(
        key_id.starts_with("notaio:v"),
        "key id names the key version"
    );
    let mut trusted = TrustedKeys::new();
    trusted.insert(&key_id, public).expect("key");
    let payload = verify_bundle(&env, &trusted, &VerifyOptions::at(Utc::now()))
        .expect("the bundle verifies against OpenBao's public key");
    assert_eq!(payload.content.policy.name, signed);

    // 2. Without OpenBao the controller fails closed: nothing is published, and the policy says
    //    why. Wait on the controller's verdict, not on the absence of a ConfigMap.
    let openbao: Api<Deployment> = Api::namespaced(client.clone(), OPENBAO_NAMESPACE);
    openbao
        .patch(
            "openbao",
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "replicas": 0 } })),
        )
        .await
        .expect("scale OpenBao to zero");
    let pods: Api<Pod> = Api::namespaced(client.clone(), OPENBAO_NAMESPACE);
    wait_for("OpenBao's pod to be gone", || {
        let pods = pods.clone();
        async move {
            let list = pods
                .list(&ListParams::default().labels("app.kubernetes.io/name=openbao"))
                .await
                .ok()?;
            list.items.is_empty().then_some(())
        }
    })
    .await;

    let refused = unique_name("unsigned");
    create_capped_policy(&client, &ns, &refused).await;
    let why = wait_ready_reason(&policies, &refused, "SigningFailed").await;
    assert!(
        why.contains("OpenBao"),
        "the reason names the signer: {why}"
    );
    assert!(
        cms.get_opt(&bundle_object_name(&refused))
            .await
            .expect("get")
            .is_none(),
        "a bundle was published without a signature from OpenBao"
    );

    // Restore. OpenBao comes back sealed; the Makefile unseals it.
    openbao
        .patch(
            "openbao",
            &PatchParams::default(),
            &Patch::Merge(json!({ "spec": { "replicas": 1 } })),
        )
        .await
        .expect("scale OpenBao back to one");
    delete_capped_policy(&client, &ns, &refused).await;
    delete_capped_policy(&client, &ns, &signed).await;
}
