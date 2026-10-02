#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{Duration, TimeZone, Utc};
use sandboxpolicy_bundle::*;
use sandboxpolicy_types::{Access, AudienceGrant, Budgets, KnobId, Lease, Step};

fn content() -> BundleContent {
    let mut knobs = BTreeMap::new();
    knobs.insert(KnobId::new("K4.1"), Step::R0);
    knobs.insert(KnobId::new("K5.1"), Step::R1);
    BundleContent {
        policy: PolicyRef {
            namespace: "sandboxes".into(),
            name: "agent-default".into(),
            uid: "uid-1".into(),
        },
        profile: ProfileRef {
            name: "hardened".into(),
            digest: "sha256:abc".into(),
        },
        knobs,
        audiences: vec![AudienceGrant {
            name: "internal-git".into(),
            access: Access::Read,
            scopes: vec!["repo:read".into()],
            ttl_seconds: Some(300),
        }],
        egress: vec![],
        tools: vec![],
        lease: Lease {
            max_duration_seconds: 14_400,
            max_concurrent: 1,
        },
        budgets: Budgets {
            max_tool_calls: 500,
            max_wall_clock_seconds: 14_400,
            max_egress_bytes: 10 * 1024 * 1024,
        },
    }
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap()
}

fn payload(version: u64) -> BundlePayload {
    BundlePayload::new(
        BundleMeta {
            version,
            issued_at: now(),
            not_after: now() + Duration::hours(1),
        },
        content(),
    )
}

fn signer() -> Ed25519Signer {
    Ed25519Signer::from_seed("test-key", [7u8; 32])
}

fn trusted(s: &Ed25519Signer) -> TrustedKeys {
    let mut k = TrustedKeys::new();
    k.insert("test-key", s.verifying_key_bytes()).unwrap();
    k
}

#[test]
fn round_trip_verifies() {
    let s = signer();
    let env = sign_bundle(&payload(3), &s).unwrap();
    let out = verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap();
    assert_eq!(out.meta.version, 3);
    assert_eq!(out.content.policy.name, "agent-default");
}

#[test]
fn digest_is_stable_and_ignores_validity_window() {
    let a = payload(1);
    let mut b = payload(9);
    b.meta.not_after += Duration::hours(5);
    assert_eq!(a.content_digest, b.content_digest);
    assert!(a.content_digest.starts_with("sha256:"));
}

#[test]
fn content_change_changes_digest() {
    let a = content();
    let mut b = content();
    b.knobs.insert(KnobId::new("K4.1"), Step::R1);
    assert_ne!(a.digest(), b.digest());
}

#[test]
fn tampered_payload_is_rejected() {
    let s = signer();
    let mut env = sign_bundle(&payload(3), &s).unwrap();
    let mut raw = STANDARD.decode(&env.payload).unwrap();
    let pos = raw.len() / 2;
    raw[pos] ^= 0x01;
    env.payload = STANDARD.encode(raw);
    let err = verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert_eq!(err, BundleError::NoValidSignature);
}

#[test]
fn unknown_key_is_rejected() {
    let s = signer();
    let env = sign_bundle(&payload(3), &s).unwrap();
    let other = Ed25519Signer::from_seed("other", [9u8; 32]);
    let mut keys = TrustedKeys::new();
    keys.insert("other", other.verifying_key_bytes()).unwrap();
    assert_eq!(
        verify_bundle(&env, &keys, &VerifyOptions::at(now())).unwrap_err(),
        BundleError::NoValidSignature
    );
}

#[test]
fn right_keyid_wrong_key_is_rejected() {
    let s = signer();
    let env = sign_bundle(&payload(3), &s).unwrap();
    let other = Ed25519Signer::from_seed("test-key", [9u8; 32]);
    assert_eq!(
        verify_bundle(&env, &trusted(&other), &VerifyOptions::at(now())).unwrap_err(),
        BundleError::NoValidSignature
    );
}

#[test]
fn expired_bundle_is_rejected() {
    let s = signer();
    let env = sign_bundle(&payload(3), &s).unwrap();
    let later = now() + Duration::hours(2);
    assert!(matches!(
        verify_bundle(&env, &trusted(&s), &VerifyOptions::at(later)).unwrap_err(),
        BundleError::Expired(_)
    ));
}

#[test]
fn future_bundle_is_rejected() {
    let s = signer();
    let env = sign_bundle(&payload(3), &s).unwrap();
    let earlier = now() - Duration::hours(1);
    assert!(matches!(
        verify_bundle(&env, &trusted(&s), &VerifyOptions::at(earlier)).unwrap_err(),
        BundleError::NotYetValid(_)
    ));
}

#[test]
fn downgrade_is_rejected() {
    let s = signer();
    let env = sign_bundle(&payload(3), &s).unwrap();
    let mut opts = VerifyOptions::at(now());
    opts.min_version = Some(4);
    assert_eq!(
        verify_bundle(&env, &trusted(&s), &opts).unwrap_err(),
        BundleError::Downgrade { got: 3, min: 4 }
    );
    opts.min_version = Some(3);
    assert!(verify_bundle(&env, &trusted(&s), &opts).is_ok());
}

#[test]
fn consistent_signature_over_inconsistent_digest_is_rejected() {
    // A buggy or malicious signer that signs a payload whose digest does not match its content.
    let s = signer();
    let mut p = payload(3);
    p.content_digest = "sha256:deadbeef".into();
    let env = sign_bundle(&p, &s).unwrap();
    assert_eq!(
        verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err(),
        BundleError::DigestMismatch
    );
}

#[test]
fn wrong_payload_type_is_rejected() {
    let s = signer();
    let mut env = sign_bundle(&payload(3), &s).unwrap();
    env.payload_type = "text/plain".into();
    assert!(matches!(
        verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err(),
        BundleError::PayloadType(_)
    ));
}

#[test]
fn bad_seed_is_rejected() {
    assert!(Ed25519Signer::from_seed_b64("k", "not base64 !!").is_err());
    assert!(Ed25519Signer::from_seed_b64("k", &STANDARD.encode([1u8; 16])).is_err());
    assert!(Ed25519Signer::from_seed_b64("k", &STANDARD.encode([1u8; 32])).is_ok());
}
