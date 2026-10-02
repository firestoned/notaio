#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The checked-in YAML must parse into the typed objects and behave under the real evaluator.

use chrono::{Duration, TimeZone, Utc};
use sandboxpolicy_api::{GrantPhase, KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::ceilings::Ceilings;
use sandboxpolicy_core::{builtin, evaluate, EvalInput};
use serde::Deserialize;

fn read(path: &str) -> String {
    let p = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn docs<T: for<'de> Deserialize<'de>>(text: &str) -> Vec<T> {
    serde_yaml_ng::Deserializer::from_str(text)
        .map(|d| T::deserialize(d).unwrap())
        .collect()
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
    let mut profile = SandboxProfile::new("hardened", builtin::profile("hardened").unwrap());
    profile.metadata.generation = Some(1);

    let ceilings = Ceilings::default();
    let now = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    let out = evaluate(&EvalInput {
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
