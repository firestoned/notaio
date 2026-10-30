#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

//! The checked-in YAML must parse into the typed objects and behave under the real evaluator.

use chrono::{Duration, TimeZone, Utc};
use sandboxpolicy_api::{ExposureCeiling, GrantPhase, KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::ceilings::Ceilings;
use sandboxpolicy_core::{builtin, evaluate, EvalInput};
use serde::Deserialize;

fn read(path: &str) -> String {
    let p = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn docs<T: for<'de> Deserialize<'de>>(text: &str) -> Vec<T> {
    serde_saphyr::from_multiple(text).unwrap()
}

#[test]
fn checked_in_profiles_match_the_code() {
    let profiles: Vec<SandboxProfile> = docs(&read("deploy/profiles/builtin.yaml"));
    assert_eq!(profiles.len(), 4);
    for p in profiles {
        let name = p.metadata.name.clone().unwrap();
        assert_eq!(p.spec, builtin::profile(&name).unwrap(), "{name} drifted");
    }
}

#[test]
fn examples_are_valid_and_compile() {
    let policy: SandboxPolicy = docs(&read("examples/sandboxpolicy-hardened.yaml")).remove(0);
    let grant: KnobGrant = docs(&read("examples/knobgrant-package-install.yaml")).remove(0);
    let ceiling: ExposureCeiling =
        docs(&read("examples/exposureceiling-agent-default.yaml")).remove(0);
    let mut profile = SandboxProfile::new("hardened", builtin::profile("hardened").unwrap());
    profile.metadata.generation = Some(1);

    let ceilings = Ceilings::default();
    let now = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    let out = evaluate(&EvalInput {
        exposure_ceiling: Some(&ceiling),
        now,
        policy: &policy,
        profile: Some(&profile),
        grants: &[grant],
        prev_version: None,
        prev_digest: None,
        prev_conditions: &[],
        bundle_lifetime: Duration::hours(1),
        ceilings: &ceilings,
    });
    assert!(out.bundle.is_some(), "{:?}", out.findings);
    assert_eq!(
        out.grants[0].phase,
        GrantPhase::Active,
        "{}",
        out.grants[0].message
    );
}

/// Schema formats the Kubernetes API server recognises in a CRD. Since Kubernetes 1.36 any other
/// format (schemars emits `uint8` to `uint64` for unsigned Rust types) draws an `unrecognized format`
/// warning on every apply. Found on the k0s test cluster, ROADMAP M1.
const KUBERNETES_FORMATS: &[&str] = &[
    "int32",
    "int64",
    "float",
    "double",
    "byte",
    "binary",
    "date",
    "date-time",
    "datetime",
    "duration",
    "password",
    "uuid",
    "uuid3",
    "uuid4",
    "uuid5",
    "email",
    "hostname",
    "ipv4",
    "ipv6",
    "cidr",
    "mac",
    "uri",
    "bsonobjectid",
    "isbn",
    "isbn10",
    "isbn13",
    "creditcard",
    "ssn",
    "hexcolor",
    "rgbcolor",
];

fn collect_formats(v: &serde_json::Value, path: &str, out: &mut Vec<(String, String)>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, child) in map {
                let here = format!("{path}.{k}");
                if k == "format" {
                    if let Some(f) = child.as_str() {
                        out.push((path.to_string(), f.to_string()));
                    }
                }
                collect_formats(child, &here, out);
            }
        }
        serde_json::Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                collect_formats(child, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn crds_use_only_formats_kubernetes_recognises() {
    let crds: Vec<serde_json::Value> = docs(&read("deploy/crds/sandbox.firestoned.io.yaml"));
    assert_eq!(crds.len(), 4);
    let mut formats = Vec::new();
    for crd in &crds {
        collect_formats(crd, crd["metadata"]["name"].as_str().unwrap(), &mut formats);
    }
    let unknown: Vec<_> = formats
        .iter()
        .filter(|(_, f)| !KUBERNETES_FORMATS.contains(&f.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "formats Kubernetes does not recognise: {unknown:#?}"
    );
}
