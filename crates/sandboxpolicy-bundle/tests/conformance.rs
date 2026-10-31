// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The checked-in conformance vectors agree with this crate's verifier and floor: every verdict in
//! `conformance/vectors.json` is what `accept` actually returns.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use sandboxpolicy_bundle::conformance::{run, suite, Expect, PAYLOAD_SCHEMA_JSON};
use sandboxpolicy_bundle::SCHEMA;

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
