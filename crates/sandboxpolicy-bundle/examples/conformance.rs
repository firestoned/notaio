// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Writes the conformance kit (roadmap 00, phase 0) into a directory: `vectors.json`, signed
//! bundles of schema v3 and v4 with the verdict each consumer must reach, and
//! `bundle-v4.schema.json`, the v4 payload's JSON Schema; and the same for audience bundles
//! (ADR-0020), `audience-vectors.json` and `audience-bundle-v1.schema.json`. The v3 payload's
//! schema, `bundle-v3.schema.json`, is frozen as checked in (ADR-0026 decision 8): the payload type
//! now models both schemas, so it is not generated again.
//!
//! ```sh
//! make conformance          # writes crates/sandboxpolicy-bundle/conformance/
//! make conformance-check    # what CI runs: fails if the checked-in files differ
//! ```
//!
//! Everything is fixed: the seeds, the clocks, the content. Ed25519 signatures are deterministic,
//! so the output is byte for byte reproducible. The keys are for these vectors only.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use jiff::{SignedDuration, Timestamp};
use sandboxpolicy_bundle::conformance::{
    AudienceSuite, Expect, PolicyCoordinates, Suite, TrustedKey, Vector,
};
use sandboxpolicy_bundle::floor::FloorEntry;
use sandboxpolicy_bundle::{
    sign_audience_bundle, sign_bundle, AudienceBundleContent, AudienceBundlePayload, AudienceRef,
    BundleContent, BundleMeta, BundlePayload, BundleTool, Ed25519Signer, Envelope, IncludeRef,
    PolicyRef, ProfileRef, Signature, Signer as _, Subjects, AUDIENCE_SCHEMA,
    MAX_BUNDLE_LIFETIME_SECONDS, PAYLOAD_TYPE_V3, PAYLOAD_TYPE_V4, SCHEMA, SCHEMA_V4,
};
use sandboxpolicy_types::{
    Access, AudienceCredential, AudienceDefinition, AudienceGrant, Backend, Budgets,
    CredentialFormat, CredentialSource, EndpointRule, KnobId, Lease, Placement, SecretRef, Step,
    ToolRef,
};
use serde_json::{json, Value};

const KEY_ID: &str = "conformance:v1";
/// The trusted key's seed. Published on purpose: it signs these vectors and nothing else.
const TRUSTED_SEED: [u8; 32] = [0x11; 32];
/// A key nobody trusts, under the same key id, so a key id alone is shown to be worth nothing.
const UNTRUSTED_SEED: [u8; 32] = [0x22; 32];
const NAMESPACE: &str = "conformance";
const NAME: &str = "agent";
const UID: &str = "00000000-0000-4000-8000-000000000001";

fn at(h: u32, m: u32) -> Timestamp {
    format!("2026-01-01T{h:02}:{m:02}:00Z")
        .parse()
        .unwrap_or_default()
}

fn content(uid: &str) -> BundleContent {
    let knobs: BTreeMap<KnobId, Step> =
        [("K4.1", Step::R0), ("K5.1", Step::R0), ("K5.3", Step::R1)]
            .into_iter()
            .map(|(k, s)| (KnobId::new(k), s))
            .collect();
    BundleContent {
        policy: PolicyRef {
            namespace: NAMESPACE.into(),
            name: NAME.into(),
            uid: uid.into(),
        },
        profile: ProfileRef {
            name: "hardened".into(),
            digest: format!("sha256:{}", "0".repeat(64)),
        },
        knobs,
        audiences: vec![AudienceGrant {
            name: "internal-git".into(),
            access: Access::Read,
            scopes: vec!["repo:read".into()],
            ttl_seconds: Some(300),
            endpoints: vec!["git.foo.io:443".into()],
            rules: vec![EndpointRule {
                method: "GET".into(),
                path: "/repos/**".into(),
            }],
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
            max_egress_bytes: 10_485_760,
        },
        subjects: None,
        includes: vec![],
        resources: None,
        sources: None,
    }
}

/// The same policy as a composite in schema v4 (ADR-0026): its groups, one include, and a tool the
/// include supplied.
fn content_v4(uid: &str) -> BundleContent {
    let mut c = content(uid);
    c.subjects = Some(Subjects {
        groups: vec!["00000000-0000-0000-0000-00000000000a".into()],
    });
    c.includes = vec![IncludeRef {
        name: "agent-tools".into(),
        uid: "00000000-0000-4000-8000-000000000003".into(),
        content_digest: format!("sha256:{}", "1".repeat(64)),
    }];
    c.tools = vec![BundleTool {
        tool: ToolRef {
            name: "linter".into(),
            manifest_digest: format!("sha256:{}", "2".repeat(64)),
        },
        from: Some("agent-tools".into()),
    }];
    c
}

/// Version 3 of the composite at 11:00, valid until 13:00, in schema v4.
fn payload_v4() -> Result<BundlePayload, String> {
    BundlePayload::new_v4(
        BundleMeta {
            version: 3,
            issued_at: at(11, 0),
            not_after: at(13, 0),
        },
        content_v4(UID),
    )
    .map_err(|e| e.to_string())
}

/// Version 3 of the policy's first incarnation, issued 11:00, valid until 13:00.
fn payload(uid: &str, version: u64, issued_at: Timestamp) -> Result<BundlePayload, String> {
    BundlePayload::new(
        BundleMeta {
            version,
            issued_at,
            not_after: issued_at + SignedDuration::from_hours(2),
        },
        content(uid),
    )
    .map_err(|e| e.to_string())
}

/// One hour past the longest lifetime a consumer accepts: a bundle signed to outlive it.
const OVERLONG: SignedDuration =
    SignedDuration::from_secs(MAX_BUNDLE_LIFETIME_SECONDS + SECONDS_PER_HOUR);
const SECONDS_PER_HOUR: i64 = 3600;

/// The first incarnation's version 3 at 11:00, signed for another policy in the same namespace.
fn other_policy() -> Result<BundlePayload, String> {
    let mut c = content(UID);
    c.policy.name = "other-agent".into();
    BundlePayload::new(
        BundleMeta {
            version: 3,
            issued_at: at(11, 0),
            not_after: at(13, 0),
        },
        c,
    )
    .map_err(|e| e.to_string())
}

/// Version 3 at 11:00, valid for longer than any consumer accepts.
fn overlong() -> Result<BundlePayload, String> {
    BundlePayload::new(
        BundleMeta {
            version: 3,
            issued_at: at(11, 0),
            not_after: at(11, 0) + OVERLONG,
        },
        content(UID),
    )
    .map_err(|e| e.to_string())
}

fn sign(p: &BundlePayload, seed: [u8; 32]) -> Result<Envelope, String> {
    sign_bundle(p, &Ed25519Signer::from_seed(KEY_ID, seed)).map_err(|e| e.to_string())
}

/// An envelope of `payload_type` around exactly `bytes`, signed by the trusted key: what a
/// misbehaving signer could produce, which `sign_bundle` will not. DSSE's pre-authentication
/// encoding, written out.
fn forge(payload_type: &str, bytes: &[u8]) -> Result<Envelope, String> {
    let mut message = format!(
        "DSSEv1 {} {payload_type} {} ",
        payload_type.len(),
        bytes.len()
    )
    .into_bytes();
    message.extend_from_slice(bytes);
    let sig = Ed25519Signer::from_seed(KEY_ID, TRUSTED_SEED)
        .sign(&message)
        .map_err(|e| e.to_string())?;
    Ok(Envelope {
        payload_type: payload_type.into(),
        payload: STANDARD.encode(bytes),
        signatures: vec![Signature {
            keyid: KEY_ID.into(),
            sig: STANDARD.encode(sig),
        }],
    })
}

fn forge_payload(payload_type: &str, p: &BundlePayload) -> Result<Envelope, String> {
    forge(
        payload_type,
        &serde_json::to_vec(p).map_err(|e| e.to_string())?,
    )
}

fn forge_json(payload_type: &str, v: &Value) -> Result<Envelope, String> {
    forge(
        payload_type,
        &serde_json::to_vec(v).map_err(|e| e.to_string())?,
    )
}

fn to_json(p: &BundlePayload) -> Result<Value, String> {
    serde_json::to_value(p).map_err(|e| e.to_string())
}

/// `sha256:` of the RFC 8785 form of `v`, as a signer that digests what it signs would compute it.
fn digest_of(v: &Value) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = sandboxpolicy_bundle::jcs::canonicalize(v).map_err(|e| e.to_string())?;
    let hex: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(format!("sha256:{hex}"))
}

/// The v4 vectors (ADR-0026 decision 8). The vectors before them stay v3 until v3 is dropped.
fn v4_vectors(v3_good: &BundlePayload) -> Result<Vec<Vector>, String> {
    let good = payload_v4()?;
    let env = sign(&good, TRUSTED_SEED)?;
    let v4 = to_json(&good)?;
    let mut v3_with_member = to_json(v3_good)?;
    v3_with_member["content"]["subjects"] =
        json!({ "groups": ["00000000-0000-0000-0000-00000000000a"] });
    let mut no_subjects = v4.clone();
    if let Some(c) = no_subjects["content"].as_object_mut() {
        c.remove("subjects");
    }
    let mut null_member = v4.clone();
    null_member["content"]["resources"] = Value::Null;
    let mut unmodelled = v4.clone();
    unmodelled["content"]["extra"] = json!(1);
    unmodelled["contentDigest"] = json!(digest_of(&unmodelled["content"])?);
    #[rustfmt::skip]
    let vectors = vec![
        accepted("v4-accepted", "A v4 bundle of a composite (subjects, an include, a tool it supplied): accepted (ADR-0026).",
            None, &env, entry(UID, 3, at(11, 0))),
        refused("v4-refused-under-v3-type", "A v4 payload under the v3 payload type: the pair does not match, refused.",
            at(12, 0), None, forge_payload(PAYLOAD_TYPE_V3, &good)?, "Schema"),
        refused("v4-refused-v3-under-v4-type", "A v3 payload under the v4 payload type: the pair does not match, refused.",
            at(12, 0), None, forge_payload(PAYLOAD_TYPE_V4, v3_good)?, "Schema"),
        refused("v4-refused-v3-with-v4-member", "A v3 payload carrying subjects: v3 is never read as v4, refused.",
            at(12, 0), None, forge_json(PAYLOAD_TYPE_V3, &v3_with_member)?, "Schema"),
        refused("v4-refused-without-subjects", "A v4 payload without subjects, which v4 always carries: refused.",
            at(12, 0), None, forge_json(PAYLOAD_TYPE_V4, &no_subjects)?, "Malformed"),
        refused("v4-refused-null-member", "A v4 payload with resources written null: only absent is canonical, so the digest of the content as signed does not match.",
            at(12, 0), None, forge_json(PAYLOAD_TYPE_V4, &null_member)?, "DigestMismatch"),
        refused("v4-refused-unmodelled-member", "A v4 payload with a member v4 does not have, digested with it: refused rather than read without it.",
            at(12, 0), None, forge_json(PAYLOAD_TYPE_V4, &unmodelled)?, "Malformed"),
    ];
    Ok(vectors)
}

/// The v4 payload's JSON Schema: the payload type's, with `subjects` required and the schema
/// pinned, since the type models v3 as well (ADR-0026 decision 8).
fn v4_schema() -> Result<Value, String> {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(BundlePayload)).map_err(|e| e.to_string())?;
    schema["properties"]["schema"]["const"] = json!(SCHEMA_V4);
    let required = schema["$defs"]["BundleContent"]["required"]
        .as_array_mut()
        .ok_or("BundleContent has no required list")?;
    required.push(json!("subjects"));
    Ok(schema)
}

fn entry(uid: &str, version: u64, issued_at: Timestamp) -> FloorEntry {
    FloorEntry {
        uid: uid.into(),
        version,
        issued_at,
    }
}

fn accepted(
    name: &str,
    description: &str,
    floor: Option<FloorEntry>,
    env: &Envelope,
    after: FloorEntry,
) -> Vector {
    Vector {
        name: name.into(),
        description: description.into(),
        now: at(12, 0),
        floor,
        min_version: None,
        envelope: env.clone(),
        expect: Expect::Accepted {
            version: after.version,
            floor_after: after,
        },
    }
}

fn refused(
    name: &str,
    description: &str,
    now: Timestamp,
    floor: Option<FloorEntry>,
    env: Envelope,
    error: &str,
) -> Vector {
    Vector {
        name: name.into(),
        description: description.into(),
        now,
        floor,
        min_version: None,
        envelope: env,
        expect: Expect::Refused {
            error: error.into(),
        },
    }
}

fn suite() -> Result<Suite, String> {
    let other_uid = "00000000-0000-4000-8000-000000000002";
    let good = payload(UID, 3, at(11, 0))?;
    let env = sign(&good, TRUSTED_SEED)?;

    let mut forged = env.clone();
    if let Some(s) = forged.signatures.first_mut() {
        let mut raw = STANDARD.decode(&s.sig).map_err(|e| e.to_string())?;
        if let Some(b) = raw.first_mut() {
            *b ^= 0x01;
        }
        s.sig = STANDARD.encode(raw);
    }
    let mut digest = good.clone();
    digest.content_digest = format!("sha256:{}", "f".repeat(64));
    let mut old_schema = good.clone();
    old_schema.schema = "sandbox.firestoned.io/bundle/v2".into();
    let mut payload_type = env.clone();
    payload_type.payload_type = "application/vnd.firestoned.sandboxpolicy.bundle.v2+json".into();
    let mut malformed = env.clone();
    malformed.payload = "not base64 at all!".into();

    let mut vectors = vec![
        accepted("accepted-first-sight", "No floor yet: a valid bundle is accepted and becomes the floor.",
            None, &env, entry(UID, 3, at(11, 0))),
        accepted("accepted-same-version", "The floor's own version again: accepted, the floor unchanged.",
            Some(entry(UID, 3, at(11, 0))), &env, entry(UID, 3, at(11, 0))),
        accepted("accepted-newer-version", "A higher version of the same incarnation: accepted, the floor moves up.",
            Some(entry(UID, 2, at(10, 0))), &env, entry(UID, 3, at(11, 0))),
        accepted("accepted-new-incarnation", "A recreated policy (new uid) issued after the floor's bundle: accepted, the floor moves to it.",
            Some(entry(other_uid, 9, at(10, 0))), &env, entry(UID, 3, at(11, 0))),
        refused("refused-downgrade", "A lower version of the incarnation the floor holds: refused.",
            at(12, 0), Some(entry(UID, 4, at(11, 30))), env.clone(), "Downgrade"),
        refused("refused-stale-incarnation", "Another incarnation, issued no later than the floor's bundle: a replay from before a recreation, refused.",
            at(12, 0), Some(entry(other_uid, 1, at(11, 30))), env.clone(), "StaleIncarnation"),
        refused("refused-forged-signature", "One bit of the signature flipped: refused.",
            at(12, 0), None, forged, "NoValidSignature"),
        refused("refused-untrusted-key", "Signed by a key nobody trusts, under the trusted key's id: refused.",
            at(12, 0), None, sign(&good, UNTRUSTED_SEED)?, "NoValidSignature"),
        refused("refused-expired", "The consumer's clock at notAfter exactly: refused.",
            at(13, 0), None, env.clone(), "Expired"),
        refused("refused-not-yet-valid", "Issued five minutes after the consumer's clock, beyond the 60 seconds of skew: refused.",
            at(12, 0), None, sign(&payload(UID, 3, at(12, 5))?, TRUSTED_SEED)?, "NotYetValid"),
        refused("refused-digest-mismatch", "Validly signed, but contentDigest is not the digest of the content: refused.",
            at(12, 0), None, sign(&digest, TRUSTED_SEED)?, "DigestMismatch"),
        refused("refused-old-schema", "Validly signed, but schema v2: refused, never read as v3.",
            at(12, 0), None, forge_payload(PAYLOAD_TYPE_V3, &old_schema)?, "Schema"),
        refused("refused-payload-type", "The envelope's payload type is v2: refused before anything else is read.",
            at(12, 0), None, payload_type, "PayloadType"),
        refused("refused-malformed", "The payload is not base64: refused.",
            at(12, 0), None, malformed, "Malformed"),
        refused("refused-audience-envelope", "A valid audience bundle (ADR-0020), signed by the trusted key: refused by its payload type, never read as a policy.",
            at(12, 0), None, sign_audience(&audience_payload(AUDIENCE_UID, 3, at(11, 0))?, TRUSTED_SEED)?, "PayloadType"),
        refused("refused-wrong-policy", "A valid bundle for another policy (conformance/other-agent), where the consumer asked for conformance/agent: refused, the floor of neither touched (ADR-0008, decision 7).",
            at(12, 0), None, sign(&other_policy()?, TRUSTED_SEED)?, "WrongSubject"),
        refused("refused-lifetime-too-long", "Validly signed and inside its window, but notAfter is 25 hours after issuedAt, past the one-day maximum: refused (ADR-0002, decision 5).",
            at(12, 0), None, sign(&overlong()?, TRUSTED_SEED)?, "LifetimeTooLong"),
    ];
    vectors.extend(v4_vectors(&good)?);
    Ok(Suite {
        schema: "sandbox.firestoned.io/conformance/v1".into(),
        bundle_schema: SCHEMA.into(),
        trusted_keys: vec![TrustedKey {
            key_id: KEY_ID.into(),
            public_key: STANDARD
                .encode(Ed25519Signer::from_seed(KEY_ID, TRUSTED_SEED).verifying_key_bytes()),
        }],
        policy: PolicyCoordinates {
            namespace: NAMESPACE.into(),
            name: NAME.into(),
        },
        vectors,
    })
}

const AUDIENCE_NAMESPACE: &str = "conformance";
const AUDIENCE_NAME: &str = "artifactory";
const AUDIENCE_UID: &str = "00000000-0000-4000-8000-000000000011";

fn audience_content(uid: &str) -> AudienceBundleContent {
    AudienceBundleContent {
        audience: AudienceRef {
            namespace: AUDIENCE_NAMESPACE.into(),
            name: AUDIENCE_NAME.into(),
            uid: uid.into(),
        },
        definition: AudienceDefinition {
            credential: AudienceCredential {
                backend: Backend::Secret,
                placement: Placement::Gateway,
                format: CredentialFormat::Basic,
                source: Some(CredentialSource::SecretRef(SecretRef {
                    name: "af-svc".into(),
                    token_key: "token".into(),
                    username_key: Some("username".into()),
                })),
                header: None,
                extra_headers: vec![],
                refresh: None,
            },
            endpoints: vec!["artifactory.foo.io:443".into()],
            rules: vec![EndpointRule {
                method: "GET".into(),
                path: "/v2/**".into(),
            }],
            max_ttl_seconds: 900,
            ca_ref: None,
        },
    }
}

fn audience_payload(
    uid: &str,
    version: u64,
    issued_at: Timestamp,
) -> Result<AudienceBundlePayload, String> {
    AudienceBundlePayload::new(
        BundleMeta {
            version,
            issued_at,
            not_after: issued_at + SignedDuration::from_hours(2),
        },
        audience_content(uid),
    )
    .map_err(|e| e.to_string())
}

fn sign_audience(p: &AudienceBundlePayload, seed: [u8; 32]) -> Result<Envelope, String> {
    sign_audience_bundle(p, &Ed25519Signer::from_seed(KEY_ID, seed)).map_err(|e| e.to_string())
}

fn audience_suite() -> Result<AudienceSuite, String> {
    let uid = AUDIENCE_UID;
    let other_uid = "00000000-0000-4000-8000-000000000012";
    let good = audience_payload(uid, 3, at(11, 0))?;
    let env = sign_audience(&good, TRUSTED_SEED)?;

    let mut forged = env.clone();
    if let Some(s) = forged.signatures.first_mut() {
        let mut raw = STANDARD.decode(&s.sig).map_err(|e| e.to_string())?;
        if let Some(b) = raw.first_mut() {
            *b ^= 0x01;
        }
        s.sig = STANDARD.encode(raw);
    }
    let mut digest = good.clone();
    digest.content_digest = format!("sha256:{}", "f".repeat(64));
    let mut policy_schema = good.clone();
    policy_schema.schema = SCHEMA.into();
    let mut malformed = env.clone();
    malformed.payload = "not base64 at all!".into();
    let mut below_min = refused(
        "refused-below-min-version",
        "Version 3 against a consumer demanding at least 4, with no floor: refused.",
        at(12, 0),
        None,
        env.clone(),
        "Downgrade",
    );
    below_min.min_version = Some(4);
    let mut other = audience_content(uid);
    other.audience.name = "nexus".into();
    let other_audience =
        AudienceBundlePayload::new(good.meta.clone(), other).map_err(|e| e.to_string())?;
    let overlong_audience = AudienceBundlePayload::new(
        BundleMeta {
            version: 3,
            issued_at: at(11, 0),
            not_after: at(11, 0) + OVERLONG,
        },
        audience_content(uid),
    )
    .map_err(|e| e.to_string())?;

    let vectors = vec![
        accepted("accepted-first-sight", "No floor yet: a valid audience bundle is accepted and becomes the floor.",
            None, &env, entry(uid, 3, at(11, 0))),
        accepted("accepted-same-version", "The floor's own version again (a re-delivery): accepted, the floor unchanged.",
            Some(entry(uid, 3, at(11, 0))), &env, entry(uid, 3, at(11, 0))),
        accepted("accepted-newer-version", "A higher version of the same incarnation: accepted, the floor moves up.",
            Some(entry(uid, 2, at(10, 0))), &env, entry(uid, 3, at(11, 0))),
        accepted("accepted-new-incarnation", "A recreated audience (new uid) issued after the floor's bundle: accepted, the floor moves to it.",
            Some(entry(other_uid, 9, at(10, 0))), &env, entry(uid, 3, at(11, 0))),
        refused("refused-downgrade", "A lower version of the incarnation the floor holds: refused.",
            at(12, 0), Some(entry(uid, 4, at(11, 30))), env.clone(), "Downgrade"),
        refused("refused-stale-incarnation", "Another incarnation, issued no later than the floor's bundle: a replay from before a recreation, refused.",
            at(12, 0), Some(entry(other_uid, 1, at(11, 30))), env.clone(), "StaleIncarnation"),
        refused("refused-policy-envelope", "A valid policy bundle, signed by the trusted key: refused by its payload type, never read as an audience.",
            at(12, 0), None, sign(&payload(UID, 3, at(11, 0))?, TRUSTED_SEED)?, "PayloadType"),
        refused("refused-forged-signature", "One bit of the signature flipped: refused.",
            at(12, 0), None, forged, "NoValidSignature"),
        refused("refused-untrusted-key", "Signed by a key nobody trusts, under the trusted key's id: refused.",
            at(12, 0), None, sign_audience(&good, UNTRUSTED_SEED)?, "NoValidSignature"),
        refused("refused-digest-mismatch", "Validly signed, but contentDigest is not the digest of the content: refused.",
            at(12, 0), None, sign_audience(&digest, TRUSTED_SEED)?, "DigestMismatch"),
        refused("refused-policy-schema", "Validly signed under the audience payload type, but the policy bundle's schema: refused.",
            at(12, 0), None, sign_audience(&policy_schema, TRUSTED_SEED)?, "Schema"),
        refused("refused-expired", "The consumer's clock at notAfter exactly: refused.",
            at(13, 0), None, env.clone(), "Expired"),
        refused("refused-not-yet-valid", "Issued five minutes after the consumer's clock, beyond the 60 seconds of skew: refused.",
            at(12, 0), None, sign_audience(&audience_payload(uid, 3, at(12, 5))?, TRUSTED_SEED)?, "NotYetValid"),
        below_min,
        refused("refused-malformed", "The payload is not base64: refused.",
            at(12, 0), None, malformed, "Malformed"),
        refused("refused-wrong-audience", "A valid bundle for another audience (conformance/nexus), where the consumer asked for conformance/artifactory: refused (ADR-0008, decision 7).",
            at(12, 0), None, sign_audience(&other_audience, TRUSTED_SEED)?, "WrongSubject"),
        refused("refused-lifetime-too-long", "Validly signed and inside its window, but notAfter is 25 hours after issuedAt, past the one-day maximum: refused (ADR-0002, decision 5).",
            at(12, 0), None, sign_audience(&overlong_audience, TRUSTED_SEED)?, "LifetimeTooLong"),
    ];
    Ok(AudienceSuite {
        schema: "sandbox.firestoned.io/conformance/v1".into(),
        bundle_schema: AUDIENCE_SCHEMA.into(),
        trusted_keys: vec![TrustedKey {
            key_id: KEY_ID.into(),
            public_key: STANDARD
                .encode(Ed25519Signer::from_seed(KEY_ID, TRUSTED_SEED).verifying_key_bytes()),
        }],
        audience: PolicyCoordinates {
            namespace: AUDIENCE_NAMESPACE.into(),
            name: AUDIENCE_NAME.into(),
        },
        vectors,
    })
}

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: conformance <dir>");
        return ExitCode::from(2);
    };
    let write = || -> Result<(), String> {
        let suite = suite()?;
        let vectors = serde_json::to_string_pretty(&suite).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("vectors.json"), vectors + "\n").map_err(|e| e.to_string())?;
        let schema = serde_json::to_string_pretty(&v4_schema()?).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("bundle-v4.schema.json"), schema + "\n")
            .map_err(|e| e.to_string())?;
        let audience =
            serde_json::to_string_pretty(&audience_suite()?).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("audience-vectors.json"), audience + "\n")
            .map_err(|e| e.to_string())?;
        let schema = schemars::schema_for!(AudienceBundlePayload);
        let schema = serde_json::to_string_pretty(&schema).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("audience-bundle-v1.schema.json"), schema + "\n")
            .map_err(|e| e.to_string())
    };
    match write() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("conformance: {e}");
            ExitCode::FAILURE
        }
    }
}
