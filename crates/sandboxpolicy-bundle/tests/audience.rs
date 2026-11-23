#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The signed audience bundle (ADR-0020, decision 9): its own payload type and schema, canonical
//! bytes pinned by a golden digest, and never interchangeable with a policy bundle.

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD as STANDARD_SIG;
use base64::Engine as _;
use jiff::{SignedDuration, Timestamp};
use sandboxpolicy_bundle::floor::{MemoryFloor, VersionFloor};
use sandboxpolicy_bundle::*;
use sandboxpolicy_types::{
    AudienceCredential, AudienceDefinition, Backend, Budgets, CredentialFormat, CredentialSource,
    EndpointRule, Lease, Placement, SecretRef,
};

fn definition() -> AudienceDefinition {
    AudienceDefinition {
        credential: AudienceCredential {
            backend: Backend::Secret,
            placement: Placement::Gateway,
            format: CredentialFormat::Basic,
            source: Some(CredentialSource::SecretRef(SecretRef {
                name: "af-team-a-svc".into(),
                token_key: "token".into(),
                username_key: Some("username".into()),
            })),
            header: None,
            extra_headers: vec![],
            refresh: None,
        },
        endpoints: vec!["artifactory.example.com:443".into()],
        rules: vec![EndpointRule {
            method: "GET".into(),
            path: "/artifactory/api/docker/app-team-a-docker/v2/**".into(),
        }],
        max_ttl_seconds: 3600,
        ca_ref: None,
    }
}

fn content() -> AudienceBundleContent {
    AudienceBundleContent {
        audience: AudienceRef {
            namespace: "team-a".into(),
            name: "artifactory".into(),
            uid: "uid-a1".into(),
        },
        definition: definition(),
    }
}

fn now() -> Timestamp {
    "2026-11-20T12:00:00Z".parse().unwrap()
}

fn meta(version: u64, issued_at: Timestamp) -> BundleMeta {
    BundleMeta {
        version,
        issued_at,
        not_after: issued_at + SignedDuration::from_mins(15),
    }
}

fn payload(version: u64) -> AudienceBundlePayload {
    AudienceBundlePayload::new(meta(version, now()), content()).unwrap()
}

fn signer() -> Ed25519Signer {
    Ed25519Signer::from_seed("test-key", [7u8; 32])
}

fn trusted(s: &Ed25519Signer) -> TrustedKeys {
    let mut k = TrustedKeys::new();
    k.insert("test-key", s.verifying_key_bytes()).unwrap();
    k
}

fn policy_payload() -> BundlePayload {
    BundlePayload::new(
        meta(1, now()),
        BundleContent {
            policy: PolicyRef {
                namespace: "team-a".into(),
                name: "artifactory".into(),
                uid: "uid-p1".into(),
            },
            profile: ProfileRef {
                name: "hardened".into(),
                digest: "sha256:abc".into(),
            },
            knobs: BTreeMap::new(),
            audiences: vec![],
            egress: vec![],
            tools: vec![],
            lease: Lease {
                max_duration_seconds: 3600,
                max_concurrent: 1,
            },
            budgets: Budgets {
                max_tool_calls: 1,
                max_wall_clock_seconds: 1,
                max_egress_bytes: 1,
            },
        },
    )
    .unwrap()
}

#[test]
fn audience_bundle_round_trips() {
    let s = signer();
    let env = sign_audience_bundle(&payload(2), &s).unwrap();
    assert_eq!(env.payload_type, AUDIENCE_PAYLOAD_TYPE);
    let out = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap();
    assert_eq!(out.meta.version, 2);
    assert_eq!(out.content.definition, definition());
    assert_eq!(out.schema, AUDIENCE_SCHEMA);
}

#[test]
fn payload_type_and_schema_are_the_adrs() {
    assert_eq!(
        AUDIENCE_PAYLOAD_TYPE,
        "application/vnd.firestoned.sandboxaudience.bundle.v1+json"
    );
    assert_eq!(AUDIENCE_SCHEMA, "sandbox.firestoned.io/audience-bundle/v1");
    assert_ne!(AUDIENCE_PAYLOAD_TYPE, PAYLOAD_TYPE);
}

/// The canonical bytes are a contract with every consumer (ADR-0005). A change here is a schema
/// change: bump `AUDIENCE_SCHEMA` and amend ADR-0020.
#[test]
fn canonical_bytes_are_pinned() {
    let bytes = String::from_utf8(content().canonical_bytes().unwrap()).unwrap();
    assert_eq!(
        bytes,
        r#"{"audience":{"name":"artifactory","namespace":"team-a","uid":"uid-a1"},"definition":{"credential":{"backend":"secret","extraHeaders":[],"format":"basic","placement":"gateway","source":{"secretRef":{"name":"af-team-a-svc","tokenKey":"token","usernameKey":"username"}}},"endpoints":["artifactory.example.com:443"],"maxTtlSeconds":3600,"rules":[{"method":"GET","path":"/artifactory/api/docker/app-team-a-docker/v2/**"}]}}"#
    );
    assert_eq!(
        content().digest().unwrap(),
        "sha256:c7ac3812e5fcf0a9e9b6872e883f02cc2f1467439cb04fb14f0f86dd5ce9e9c0"
    );
}

#[test]
fn a_policy_bundle_never_verifies_as_an_audience_bundle() {
    let s = signer();
    let env = sign_bundle(&policy_payload(), &s).unwrap();
    let err = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert!(matches!(err, BundleError::PayloadType(_)), "{err}");
}

#[test]
fn an_audience_bundle_never_verifies_as_a_policy_bundle() {
    let s = signer();
    let env = sign_audience_bundle(&payload(1), &s).unwrap();
    let err = verify_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert!(matches!(err, BundleError::PayloadType(_)), "{err}");
}

/// Relabelling the envelope does not help: DSSE signs the payload type, so the signature fails.
#[test]
fn a_relabelled_envelope_has_no_valid_signature() {
    let s = signer();
    let mut env = sign_bundle(&policy_payload(), &s).unwrap();
    env.payload_type = AUDIENCE_PAYLOAD_TYPE.to_string();
    let err = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert_eq!(err, BundleError::NoValidSignature);
}

#[test]
fn tampered_definition_is_rejected() {
    let s = signer();
    let mut p = payload(1);
    let env = sign_audience_bundle(&p, &s).unwrap();
    p.content.definition.endpoints = vec!["elsewhere.example.com:443".into()];
    let forged = audience_envelope(
        &serde_json::to_vec(&p).unwrap(),
        "test-key",
        &STANDARD_SIG.decode(&env.signatures[0].sig).unwrap(),
    );
    let err = verify_audience_bundle(&forged, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert_eq!(err, BundleError::NoValidSignature);
}

#[test]
fn a_digest_that_does_not_match_the_content_is_rejected() {
    let s = signer();
    let mut p = payload(1);
    p.content_digest = "sha256:00".into();
    let env = sign_audience_bundle(&p, &s).unwrap();
    let err = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert_eq!(err, BundleError::DigestMismatch);
}

#[test]
fn a_wrong_schema_is_rejected() {
    let s = signer();
    let mut p = payload(1);
    p.schema = SCHEMA.to_string();
    let env = sign_audience_bundle(&p, &s).unwrap();
    let err = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert!(matches!(err, BundleError::Schema(_)), "{err}");
}

#[test]
fn an_expired_audience_bundle_is_rejected() {
    let s = signer();
    let env = sign_audience_bundle(&payload(1), &s).unwrap();
    let later = now() + SignedDuration::from_hours(1);
    let err = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(later)).unwrap_err();
    assert!(matches!(err, BundleError::Expired(_)), "{err}");
}

#[test]
fn split_signing_produces_the_same_envelope() {
    let s = signer();
    let p = payload(4);
    let input = audience_signing_input(&p).unwrap();
    let sig = s.sign(&input.message).unwrap();
    let split = audience_envelope(&input.payload, s.key_id(), &sig);
    assert_eq!(split, sign_audience_bundle(&p, &s).unwrap());
}

/// The floor is per audience namespace and name (ADR-0008), and an audience never shares a floor
/// entry with a policy of the same namespace and name, even in one store.
#[test]
fn the_floor_refuses_an_older_audience_bundle_and_is_apart_from_policies() {
    let s = signer();
    let keys = trusted(&s);
    let mut floor = MemoryFloor::default();

    let v2 = sign_audience_bundle(&payload(2), &s).unwrap();
    floor::accept_audience(&v2, &keys, now(), "team-a", "artifactory", &mut floor).unwrap();

    let v1 = sign_audience_bundle(&payload(1), &s).unwrap();
    let err =
        floor::accept_audience(&v1, &keys, now(), "team-a", "artifactory", &mut floor).unwrap_err();
    assert_eq!(err, BundleError::Downgrade { got: 1, min: 2 });

    // A policy named like the audience starts from its own floor.
    let policy = sign_bundle(&policy_payload(), &s).unwrap();
    floor::accept(&policy, &keys, now(), "team-a", "artifactory", &mut floor).unwrap();
}

#[test]
fn a_recreated_audience_restarts_its_floor() {
    let s = signer();
    let keys = trusted(&s);
    let mut floor = MemoryFloor::default();
    let v5 = sign_audience_bundle(&payload(5), &s).unwrap();
    floor::accept_audience(&v5, &keys, now(), "team-a", "artifactory", &mut floor).unwrap();

    let mut c = content();
    c.audience.uid = "uid-a2".into();
    let later = now() + SignedDuration::from_mins(1);
    let recreated = AudienceBundlePayload::new(meta(1, later), c).unwrap();
    let env = sign_audience_bundle(&recreated, &s).unwrap();
    floor::accept_audience(&env, &keys, later, "team-a", "artifactory", &mut floor).unwrap();

    // The old incarnation is now a replay.
    let err =
        floor::accept_audience(&v5, &keys, later, "team-a", "artifactory", &mut floor).unwrap_err();
    assert!(matches!(err, BundleError::StaleIncarnation { .. }), "{err}");
}

#[test]
fn another_audiences_bundle_is_refused_and_never_touches_the_floor() {
    // ADR-0008, decision 7, for audiences: the consumer asked for team-a/nexus.
    let s = signer();
    let keys = trusted(&s);
    let mut floor = MemoryFloor::default();
    let env = sign_audience_bundle(&payload(2), &s).unwrap();
    let err =
        floor::accept_audience(&env, &keys, now(), "team-a", "nexus", &mut floor).unwrap_err();
    assert_eq!(
        err,
        BundleError::WrongSubject {
            expected: "team-a/nexus".into(),
            got: "team-a/artifactory".into(),
        }
    );
    assert!(floor
        .get("team-a", &floor::audience_floor_name("nexus"))
        .is_none());
    assert!(floor
        .get("team-a", &floor::audience_floor_name("artifactory"))
        .is_none());
}

#[test]
fn an_audience_bundle_signed_to_outlive_the_maximum_lifetime_is_rejected() {
    let s = signer();
    let max = SignedDuration::from_secs(MAX_BUNDLE_LIFETIME_SECONDS);
    let p = AudienceBundlePayload::new(
        BundleMeta {
            version: 1,
            issued_at: now(),
            not_after: now() + max + SignedDuration::from_secs(1),
        },
        content(),
    )
    .unwrap();
    let env = sign_audience_bundle(&p, &s).unwrap();
    let err = verify_audience_bundle(&env, &trusted(&s), &VerifyOptions::at(now())).unwrap_err();
    assert!(
        matches!(err, BundleError::LifetimeTooLong { .. }),
        "{err:?}"
    );
}
