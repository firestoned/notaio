// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The checked-in conformance vectors agree with this crate's verifier and floor: every verdict in
//! `conformance/vectors.json` is what `accept` actually returns, and every verdict in
//! `conformance/audience-vectors.json` what the audience verifier and floor return.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use sandboxpolicy_bundle::conformance::{
    audience_suite, run, run_audience, suite, Expect, AUDIENCE_PAYLOAD_SCHEMA_JSON,
    PAYLOAD_SCHEMA_JSON, PAYLOAD_SCHEMA_V4_JSON,
};
use sandboxpolicy_bundle::{AUDIENCE_PAYLOAD_TYPE, AUDIENCE_SCHEMA, SCHEMA, SCHEMA_V4};

#[test]
fn every_vector_gets_its_verdict() {
    let s = suite().expect("vectors.json parses");
    let keys = s.keys().expect("the suite's keys load");
    let failures: Vec<String> = s
        .vectors
        .iter()
        .filter_map(|v| run(&s, v, &keys).err().map(|e| format!("{}: {e}", v.name)))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_suite_covers_every_refusal_a_consumer_can_see() {
    let s = suite().unwrap();
    let names: BTreeSet<&str> = s.vectors.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names.len(), s.vectors.len(), "vector names are unique");
    let refusals: BTreeSet<&str> = s
        .vectors
        .iter()
        .filter_map(|v| match &v.expect {
            Expect::Refused { error } => Some(error.as_str()),
            Expect::Accepted { .. } => None,
        })
        .collect();
    for e in [
        "PayloadType",
        "Malformed",
        "NoValidSignature",
        "Schema",
        "DigestMismatch",
        "NotYetValid",
        "Expired",
        "Downgrade",
        "StaleIncarnation",
    ] {
        assert!(refusals.contains(e), "no vector is refused with {e}");
    }
    assert!(s
        .vectors
        .iter()
        .any(|v| matches!(v.expect, Expect::Accepted { .. })));
}

#[test]
fn the_suite_and_its_schema_are_for_this_bundle_schema() {
    let s = suite().unwrap();
    assert_eq!(s.bundle_schema, SCHEMA);
    let schema: serde_json::Value = serde_json::from_str(PAYLOAD_SCHEMA_JSON).unwrap();
    assert_eq!(schema["title"], "BundlePayload");
    // The v3 shape is in the schema: an audience's endpoints and rules.
    let text = PAYLOAD_SCHEMA_JSON;
    assert!(text.contains("\"endpoints\"") && text.contains("\"rules\""));
}

#[test]
fn the_v4_vectors_cover_adr_0026_decision_8() {
    let s = suite().unwrap();
    let verdict = |name: &str| {
        s.vectors
            .iter()
            .find(|v| v.name == name)
            .map(|v| v.expect.clone())
            .unwrap_or_else(|| panic!("no vector {name}"))
    };
    assert!(matches!(verdict("v4-accepted"), Expect::Accepted { .. }));
    for (name, error) in [
        ("v4-refused-under-v3-type", "Schema"),
        ("v4-refused-v3-under-v4-type", "Schema"),
        ("v4-refused-v3-with-v4-member", "Schema"),
        ("v4-refused-without-subjects", "Malformed"),
        ("v4-refused-null-member", "DigestMismatch"),
        ("v4-refused-unmodelled-member", "Malformed"),
    ] {
        assert_eq!(
            verdict(name),
            Expect::Refused {
                error: error.into()
            },
            "{name}"
        );
    }
}

#[test]
fn the_v4_schema_requires_subjects_and_pins_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(PAYLOAD_SCHEMA_V4_JSON).unwrap();
    assert_eq!(schema["title"], "BundlePayload");
    assert_eq!(schema["properties"]["schema"]["const"], SCHEMA_V4);
    let required = schema["$defs"]["BundleContent"]["required"]
        .as_array()
        .unwrap();
    assert!(required.iter().any(|r| r == "subjects"));
    // The frozen v3 schema knows nothing of v4.
    assert!(!PAYLOAD_SCHEMA_JSON.contains("\"subjects\""));
}

// The audience bundle vectors (ADR-0020).

#[test]
fn every_audience_vector_gets_its_verdict() {
    let s = audience_suite().expect("audience-vectors.json parses");
    let keys = s.keys().expect("the suite's keys load");
    let failures: Vec<String> = s
        .vectors
        .iter()
        .filter_map(|v| {
            run_audience(&s, v, &keys)
                .err()
                .map(|e| format!("{}: {e}", v.name))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_audience_suite_covers_every_case_a_consumer_can_see() {
    let s = audience_suite().unwrap();
    let names: BTreeSet<&str> = s.vectors.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names.len(), s.vectors.len(), "vector names are unique");
    for name in [
        "accepted-first-sight",
        "accepted-same-version",
        "accepted-newer-version",
        "accepted-new-incarnation",
        "refused-downgrade",
        "refused-stale-incarnation",
        "refused-policy-envelope",
        "refused-untrusted-key",
        "refused-forged-signature",
        "refused-digest-mismatch",
        "refused-policy-schema",
        "refused-expired",
        "refused-not-yet-valid",
        "refused-below-min-version",
        "refused-malformed",
    ] {
        assert!(names.contains(name), "no audience vector {name}");
    }
    assert_eq!(s.bundle_schema, AUDIENCE_SCHEMA);
    let schema: serde_json::Value = serde_json::from_str(AUDIENCE_PAYLOAD_SCHEMA_JSON).unwrap();
    assert_eq!(schema["title"], "AudienceBundlePayload");
}

/// The audience envelope down the policy path is refused by its payload type: the other half of
/// "neither kind verifies as the other".
#[test]
fn the_policy_suite_refuses_an_audience_envelope() {
    let s = suite().unwrap();
    let v = s
        .vectors
        .iter()
        .find(|v| v.name == "refused-audience-envelope")
        .expect("a vector with an audience envelope");
    assert_eq!(v.envelope.payload_type, AUDIENCE_PAYLOAD_TYPE);
    assert_eq!(
        v.expect,
        Expect::Refused {
            error: "PayloadType".into()
        }
    );
}
