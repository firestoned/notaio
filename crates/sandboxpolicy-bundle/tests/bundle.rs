#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

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
    .unwrap()
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
    assert_ne!(a.digest().unwrap(), b.digest().unwrap());
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

/// The content's canonical bytes and digest for `content()`, produced by an independent
/// implementation rather than this crate: Python's `json.dumps(sort_keys=True,
/// separators=(",", ":"), ensure_ascii=False)`, which equals RFC 8785 for integer-only content
/// with BMP keys (ADR-0005).
const FIXTURE_CANONICAL: &str = r#"{"audiences":[{"access":"Read","name":"internal-git","scopes":["repo:read"],"ttlSeconds":300}],"budgets":{"maxEgressBytes":10485760,"maxToolCalls":500,"maxWallClockSeconds":14400},"egress":[],"knobs":{"K4.1":"R0","K5.1":"R1"},"lease":{"maxConcurrent":1,"maxDurationSeconds":14400},"policy":{"name":"agent-default","namespace":"sandboxes","uid":"uid-1"},"profile":{"digest":"sha256:abc","name":"hardened"},"tools":[]}"#;
const FIXTURE_DIGEST: &str =
    "sha256:b25c43a63a17fc5ddb34748d7f80523fbca0da2ec534ac1ae80ad9aad0f8eef7";

#[test]
fn canonical_bytes_are_rfc_8785() {
    let bytes = content().canonical_bytes().unwrap();
    assert_eq!(String::from_utf8(bytes).unwrap(), FIXTURE_CANONICAL);
    assert_eq!(content().digest().unwrap(), FIXTURE_DIGEST);
}

#[test]
fn schema_is_v2() {
    // ADR-0005 changed the canonical form, which is a schema change (CLAUDE.md).
    assert_eq!(SCHEMA, "sandbox.firestoned.io/bundle/v2");
    assert_eq!(
        PAYLOAD_TYPE,
        "application/vnd.firestoned.sandboxpolicy.bundle.v2+json"
    );
}

#[test]
fn content_beyond_the_safe_integer_range_is_refused() {
    // ADR-0005, decision 2. Removing the bound makes this test fail.
    let mut c = content();
    c.budgets.max_egress_bytes = 1 << 53;
    assert!(matches!(c.digest(), Err(BundleError::NonCanonical(_))));
    let meta = BundleMeta {
        version: 1,
        issued_at: now(),
        not_after: now() + Duration::hours(1),
    };
    assert!(matches!(
        BundlePayload::new(meta, c),
        Err(BundleError::NonCanonical(_))
    ));
}

#[test]
fn verification_refuses_content_without_a_canonical_form() {
    // A buggy or malicious signer signs content outside the safe range. The consumer must refuse
    // it rather than compute a digest another implementation would compute differently.
    let s = signer();
    let mut p = payload(3);
    p.content.budgets.max_egress_bytes = 1 << 53;
    let env = sign_bundle(&p, &s).unwrap();
    assert!(matches!(
        verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())),
        Err(BundleError::NonCanonical(_))
    ));
}

#[test]
fn split_signing_produces_the_same_envelope() {
    // An async signer (OpenBao transit, ADR-0004) signs the message out of band and assembles the
    // envelope afterwards. That must be byte for byte what sign_bundle produces.
    let s = signer();
    let p = payload(3);
    let input = signing_input(&p).unwrap();
    let sig = s.sign(&input.message).unwrap();
    let env = envelope(&input.payload, s.key_id(), &sig);
    assert_eq!(env, sign_bundle(&p, &s).unwrap());
    verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap();
}
