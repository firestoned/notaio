#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Bundle schema v4 beside v3 (ADR-0026): the v4 members, the pairing of payload type and schema,
//! the refusals a reader of both makes, and the payload type carried through out-of-band signing.

use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use jiff::{SignedDuration, Timestamp};
use sandboxpolicy_bundle::*;
use sandboxpolicy_types::{Access, AudienceGrant, Budgets, KnobId, Lease, Step, ToolRef};
use serde_json::{json, Value};

fn now() -> Timestamp {
    "2026-09-30T12:00:00Z".parse().unwrap()
}

fn meta(version: u64) -> BundleMeta {
    BundleMeta {
        version,
        issued_at: now(),
        not_after: now() + SignedDuration::from_hours(1),
    }
}

/// The same content as `tests/bundle.rs`, in v3 form: no v4 member.
fn content_v3() -> BundleContent {
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
            endpoints: vec!["git.internal.example:443".into()],
            rules: vec![],
        }
        .into()],
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
        subjects: None,
        includes: vec![],
        resources: None,
        sources: None,
    }
}

/// A composite's v4 content: its groups, one include, and a tool that include supplied.
fn content_v4() -> BundleContent {
    let mut c = content_v3();
    c.subjects = Some(Subjects {
        groups: vec!["algo-devs".into(), "sre".into()],
    });
    c.includes = vec![IncludeRef {
        name: "agent-tools".into(),
        uid: "uid-2".into(),
        content_digest: "sha256:def".into(),
    }];
    c.tools = vec![BundleTool {
        tool: ToolRef {
            name: "linter".into(),
            manifest_digest: "sha256:0123".into(),
        },
        from: Some("agent-tools".into()),
    }];
    c
}

fn signer() -> Ed25519Signer {
    Ed25519Signer::from_seed("test-key", [7u8; 32])
}

fn trusted() -> TrustedKeys {
    let mut k = TrustedKeys::new();
    k.insert("test-key", signer().verifying_key_bytes())
        .unwrap();
    k
}

fn verify(env: &Envelope) -> Result<BundlePayload, BundleError> {
    verify_bundle(env, &trusted(), &VerifyOptions::at(now()))
}

/// An envelope of `payload_type` around exactly `bytes`, signed by the trusted key: what a
/// misbehaving signer could produce. DSSE's pre-authentication encoding, written out here rather
/// than taken from the crate under test.
fn forge(payload_type: &str, bytes: &[u8]) -> Envelope {
    let mut message = format!(
        "DSSEv1 {} {} {} ",
        payload_type.len(),
        payload_type,
        bytes.len()
    )
    .into_bytes();
    message.extend_from_slice(bytes);
    let sig = signer().sign(&message).unwrap();
    Envelope {
        payload_type: payload_type.into(),
        payload: STANDARD.encode(bytes),
        signatures: vec![Signature {
            keyid: "test-key".into(),
            sig: STANDARD.encode(sig),
        }],
    }
}

/// A valid payload of `schema` as JSON, to be edited and forged.
fn payload_json(p: &BundlePayload) -> Value {
    serde_json::to_value(p).unwrap()
}

fn forge_json(payload_type: &str, v: &Value) -> Envelope {
    forge(payload_type, &serde_json::to_vec(v).unwrap())
}

#[test]
fn the_v4_names_are_the_ones_adr_0026_fixes() {
    assert_eq!(SCHEMA_V3, "sandbox.firestoned.io/bundle/v3");
    assert_eq!(SCHEMA_V4, "sandbox.firestoned.io/bundle/v4");
    assert_eq!(
        PAYLOAD_TYPE_V3,
        "application/vnd.firestoned.sandboxpolicy.bundle.v3+json"
    );
    assert_eq!(
        PAYLOAD_TYPE_V4,
        "application/vnd.firestoned.sandboxpolicy.bundle.v4+json"
    );
    // Until release N+2, what `BundlePayload::new` writes is still v3.
    assert_eq!(SCHEMA, SCHEMA_V3);
    assert_eq!(PAYLOAD_TYPE, PAYLOAD_TYPE_V3);
    for v in [SchemaVersion::V3, SchemaVersion::V4] {
        assert_eq!(SchemaVersion::from_schema(v.schema()), Some(v));
        assert_eq!(SchemaVersion::from_payload_type(v.payload_type()), Some(v));
    }
    assert_eq!(
        SchemaVersion::from_schema("sandbox.firestoned.io/bundle/v2"),
        None
    );
}

#[test]
fn a_v4_bundle_round_trips_and_says_it_is_v4() {
    let p = BundlePayload::new_v4(meta(4), content_v4()).unwrap();
    assert_eq!(p.schema, SCHEMA_V4);
    let env = sign_bundle(&p, &signer()).unwrap();
    assert_eq!(env.payload_type, PAYLOAD_TYPE_V4);
    let out = verify(&env).unwrap();
    assert_eq!(out.schema_version(), Some(SchemaVersion::V4));
    assert_eq!(
        out.content.subjects.as_ref().unwrap().groups,
        ["algo-devs", "sre"]
    );
    assert_eq!(out.content.includes[0].name, "agent-tools");
    assert_eq!(out.content.tools[0].from.as_deref(), Some("agent-tools"));
    // The entry still reads as the shared type.
    assert_eq!(out.content.tools[0].name, "linter");
    assert_eq!(out.content.audiences[0].from, None);
}

#[test]
fn a_v3_bundle_still_round_trips_and_says_it_is_v3() {
    let p = BundlePayload::new(meta(3), content_v3()).unwrap();
    let env = sign_bundle(&p, &signer()).unwrap();
    assert_eq!(env.payload_type, PAYLOAD_TYPE_V3);
    let out = verify(&env).unwrap();
    assert_eq!(out.schema_version(), Some(SchemaVersion::V3));
    assert!(out.content.subjects.is_none());
}

#[test]
fn the_payload_type_travels_with_the_payload_through_out_of_band_signing() {
    // ADR-0026 decision 3: OpenBao transit signs `message` and the envelope is assembled after. A
    // v4 payload must come out under the v4 type, or no reader would verify it.
    let s = signer();
    let p = BundlePayload::new_v4(meta(4), content_v4()).unwrap();
    let input = signing_input(&p).unwrap();
    assert_eq!(input.payload_type, PAYLOAD_TYPE_V4);
    let sig = s.sign(&input.message).unwrap();
    let env = envelope(&input, s.key_id(), &sig);
    assert_eq!(env, sign_bundle(&p, &s).unwrap());
    assert_eq!(
        verify(&env).unwrap().schema_version(),
        Some(SchemaVersion::V4)
    );
}

#[test]
fn new_v3_refuses_content_with_a_v4_member() {
    for c in [
        BundleContent {
            subjects: Some(Subjects { groups: vec![] }),
            ..content_v3()
        },
        BundleContent {
            includes: content_v4().includes,
            ..content_v3()
        },
        BundleContent {
            tools: content_v4().tools,
            ..content_v3()
        },
        BundleContent {
            resources: Some(Resources {
                vcpus: 2,
                memory_mib: 4096,
                disk_gib: 20,
            }),
            ..content_v3()
        },
        BundleContent {
            sources: Some(Sources {
                policy: Some("policies@0123abc".into()),
                ceiling: None,
                grants: BTreeMap::new(),
            }),
            ..content_v3()
        },
    ] {
        assert!(matches!(
            BundlePayload::new_v3(meta(3), c),
            Err(BundleError::Malformed(_))
        ));
    }
}

#[test]
fn new_v4_refuses_content_without_subjects() {
    assert!(matches!(
        BundlePayload::new_v4(meta(4), content_v3()),
        Err(BundleError::Malformed(_))
    ));
}

#[test]
fn a_payload_type_and_a_schema_that_do_not_pair_are_refused() {
    let v4 = payload_json(&BundlePayload::new_v4(meta(4), content_v4()).unwrap());
    let v3 = payload_json(&BundlePayload::new(meta(3), content_v3()).unwrap());
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V3, &v4)),
        Err(BundleError::Schema(_))
    ));
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V4, &v3)),
        Err(BundleError::Schema(_))
    ));
}

#[test]
fn a_v3_payload_carrying_any_v4_member_is_refused() {
    // ADR-0026 decision 4: v3 is never read as v4, whatever it carries.
    let v3 = payload_json(&BundlePayload::new(meta(3), content_v3()).unwrap());
    let members = [
        ("subjects", json!({ "groups": ["algo-devs"] })),
        (
            "includes",
            json!([{ "name": "a", "uid": "u", "contentDigest": "sha256:d" }]),
        ),
        (
            "resources",
            json!({ "vcpus": 1, "memoryMiB": 2048, "diskGiB": 10 }),
        ),
        ("sources", json!({ "policy": "policies@0123abc" })),
    ];
    for (k, v) in members {
        let mut p = v3.clone();
        p["content"][k] = v;
        assert!(
            matches!(
                verify(&forge_json(PAYLOAD_TYPE_V3, &p)),
                Err(BundleError::Schema(_))
            ),
            "{k}"
        );
    }
    let mut p = v3;
    p["content"]["audiences"][0]["from"] = json!("an-include");
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V3, &p)),
        Err(BundleError::Schema(_))
    ));
}

#[test]
fn a_v4_payload_without_subjects_is_malformed() {
    let mut p = payload_json(&BundlePayload::new_v4(meta(4), content_v4()).unwrap());
    p["content"].as_object_mut().unwrap().remove("subjects");
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V4, &p)),
        Err(BundleError::Malformed(_))
    ));
}

#[test]
fn an_optional_member_written_null_fails_the_digest() {
    // Absent and null must not both verify: only absent is the canonical form (ADR-0026 decision 1).
    let mut p = payload_json(&BundlePayload::new_v4(meta(4), content_v4()).unwrap());
    p["content"]["resources"] = Value::Null;
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V4, &p)),
        Err(BundleError::DigestMismatch)
    ));
}

#[test]
fn an_unknown_schema_under_a_known_type_is_refused() {
    let mut p = payload_json(&BundlePayload::new_v4(meta(4), content_v4()).unwrap());
    p["schema"] = json!("sandbox.firestoned.io/bundle/v5");
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V4, &p)),
        Err(BundleError::Schema(_))
    ));
}

#[test]
fn a_consumer_that_enforces_subjects_can_refuse_v3() {
    let opts = VerifyOptions {
        min_schema: SchemaVersion::V4,
        ..VerifyOptions::at(now())
    };
    let v3 = sign_bundle(
        &BundlePayload::new(meta(3), content_v3()).unwrap(),
        &signer(),
    )
    .unwrap();
    let v4 = sign_bundle(
        &BundlePayload::new_v4(meta(4), content_v4()).unwrap(),
        &signer(),
    )
    .unwrap();
    assert!(matches!(
        verify_bundle(&v3, &trusted(), &opts),
        Err(BundleError::Schema(_))
    ));
    verify_bundle(&v4, &trusted(), &opts).unwrap();
    // The default reads both.
    assert_eq!(VerifyOptions::at(now()).min_schema, SchemaVersion::V3);
}

/// `content_v4()`'s canonical bytes and digest, from Python's `json.dumps(sort_keys=True,
/// separators=(",", ":"), ensure_ascii=False)` over the same members, which equals RFC 8785 for
/// integer-only content with BMP keys (ADR-0005). Not computed by this crate.
const V4_CANONICAL: &str = r#"{"audiences":[{"access":"Read","endpoints":["git.internal.example:443"],"name":"internal-git","rules":[],"scopes":["repo:read"],"ttlSeconds":300}],"budgets":{"maxEgressBytes":10485760,"maxToolCalls":500,"maxWallClockSeconds":14400},"egress":[],"includes":[{"contentDigest":"sha256:def","name":"agent-tools","uid":"uid-2"}],"knobs":{"K4.1":"R0","K5.1":"R1"},"lease":{"maxConcurrent":1,"maxDurationSeconds":14400},"policy":{"name":"agent-default","namespace":"sandboxes","uid":"uid-1"},"profile":{"digest":"sha256:abc","name":"hardened"},"subjects":{"groups":["algo-devs","sre"]},"tools":[{"from":"agent-tools","manifestDigest":"sha256:0123","name":"linter"}]}"#;
const V4_DIGEST: &str = "sha256:95bc5bf720b68997fe729d527d0531041682fa41282fef9aeefad597a5d5ef92";

#[test]
fn v4_canonical_bytes_are_rfc_8785() {
    let c = content_v4();
    assert_eq!(
        String::from_utf8(c.canonical_bytes().unwrap()).unwrap(),
        V4_CANONICAL
    );
    assert_eq!(c.digest().unwrap(), V4_DIGEST);
}

#[test]
fn a_member_the_schema_does_not_model_is_refused_even_under_its_own_digest() {
    // A trusted but misbehaving signer adds a member and digests it too. A verifier hashing the
    // content as signed would accept it; the types would drop it. Refused, not read without it.
    let mut p = payload_json(&BundlePayload::new_v4(meta(4), content_v4()).unwrap());
    p["content"]["extra"] = json!(1);
    let bytes = jcs::canonicalize(&p["content"]).unwrap();
    p["contentDigest"] = json!(format!("sha256:{}", sha256_hex(&bytes)));
    assert!(matches!(
        verify(&forge_json(PAYLOAD_TYPE_V4, &p)),
        Err(BundleError::Malformed(_))
    ));
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
