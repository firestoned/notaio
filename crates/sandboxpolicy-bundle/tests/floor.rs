#![allow(clippy::unwrap_used, clippy::expect_used)]
// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0

//! The consumer's persisted version floor (ADR-0008).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, TimeZone, Utc};
use sandboxpolicy_bundle::floor::{accept, FileFloor, MemoryFloor, VersionFloor};
use sandboxpolicy_bundle::*;
use sandboxpolicy_types::{Access, AudienceGrant, Budgets, KnobId, Lease, Step};

const NS: &str = "sandboxes";
const NAME: &str = "agent-default";

fn t(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 21, hour, 0, 0).unwrap()
}

fn signer() -> Ed25519Signer {
    Ed25519Signer::from_seed("test-key", [7u8; 32])
}

fn keys() -> TrustedKeys {
    let mut k = TrustedKeys::new();
    k.insert("test-key", signer().verifying_key_bytes())
        .unwrap();
    k
}

/// A signed bundle for incarnation `uid` at `version`, issued at `issued`, valid for a day.
fn bundle(uid: &str, version: u64, issued: DateTime<Utc>) -> Envelope {
    let mut knobs = BTreeMap::new();
    knobs.insert(KnobId::new("K4.1"), Step::R0);
    let content = BundleContent {
        policy: PolicyRef {
            namespace: NS.into(),
            name: NAME.into(),
            uid: uid.into(),
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
    };
    let meta = BundleMeta {
        version,
        issued_at: issued,
        not_after: issued + Duration::days(1),
    };
    sign_bundle(&BundlePayload::new(meta, content).unwrap(), &signer()).unwrap()
}

/// A fresh, empty directory under cargo's per-test temporary directory.
fn dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_first_bundle_is_accepted_and_recorded() {
    let mut floor = MemoryFloor::default();
    let p = accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap();
    assert_eq!(p.meta.version, 3);
    let e = floor.get(NS, NAME).expect("recorded");
    assert_eq!((e.uid.as_str(), e.version, e.issued_at), ("uid-1", 3, t(1)));
}

#[test]
fn same_incarnation_lower_version_is_a_downgrade() {
    let mut floor = MemoryFloor::default();
    accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap();
    let err = accept(&bundle("uid-1", 2, t(1)), &keys(), t(3), &mut floor).unwrap_err();
    assert_eq!(err, BundleError::Downgrade { got: 2, min: 3 });
    assert_eq!(
        floor.get(NS, NAME).unwrap().version,
        3,
        "the floor never goes down"
    );
}

#[test]
fn same_incarnation_same_version_is_a_redelivery() {
    let mut floor = MemoryFloor::default();
    accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap();
    accept(&bundle("uid-1", 3, t(2)), &keys(), t(3), &mut floor).unwrap();
    assert_eq!(floor.get(NS, NAME).unwrap().version, 3);
}

#[test]
fn same_incarnation_higher_version_raises_the_floor() {
    let mut floor = MemoryFloor::default();
    accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap();
    accept(&bundle("uid-1", 4, t(3)), &keys(), t(4), &mut floor).unwrap();
    let e = floor.get(NS, NAME).unwrap();
    assert_eq!((e.version, e.issued_at), (4, t(3)));
}

#[test]
fn a_newer_incarnation_resets_the_floor() {
    // The policy was deleted and recreated: new uid, versions restart at 1.
    let mut floor = MemoryFloor::default();
    accept(&bundle("uid-1", 7, t(1)), &keys(), t(2), &mut floor).unwrap();
    accept(&bundle("uid-2", 1, t(3)), &keys(), t(4), &mut floor).unwrap();
    let e = floor.get(NS, NAME).unwrap();
    assert_eq!((e.uid.as_str(), e.version), ("uid-2", 1));
}

#[test]
fn an_older_incarnation_is_a_replay() {
    let mut floor = MemoryFloor::default();
    accept(&bundle("uid-2", 1, t(3)), &keys(), t(4), &mut floor).unwrap();
    // Still signed and still inside its validity window, but from before the recreation.
    let err = accept(&bundle("uid-1", 7, t(1)), &keys(), t(4), &mut floor).unwrap_err();
    assert!(
        matches!(err, BundleError::StaleIncarnation { .. }),
        "{err:?}"
    );
    assert_eq!(floor.get(NS, NAME).unwrap().uid, "uid-2");
}

#[test]
fn a_bundle_that_fails_verification_never_touches_the_floor() {
    let mut floor = MemoryFloor::default();
    let mut env = bundle("uid-1", 3, t(1));
    env.signatures[0].keyid = "someone-else".into();
    assert!(accept(&env, &keys(), t(2), &mut floor).is_err());
    assert!(floor.get(NS, NAME).is_none());
}

#[test]
fn the_file_floor_survives_a_restart() {
    let path = dir("floor-restart").join("floor.json");
    {
        let mut floor = FileFloor::open(&path).unwrap();
        accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap();
    }
    // A new process: the floor comes back from disk and still refuses the downgrade.
    let mut floor = FileFloor::open(&path).unwrap();
    let err = accept(&bundle("uid-1", 2, t(1)), &keys(), t(3), &mut floor).unwrap_err();
    assert_eq!(err, BundleError::Downgrade { got: 2, min: 3 });
}

#[test]
fn a_missing_file_is_an_empty_floor() {
    let floor = FileFloor::open(&dir("floor-missing").join("floor.json")).unwrap();
    assert!(floor.get(NS, NAME).is_none());
}

#[test]
fn a_corrupt_file_is_an_error_not_an_empty_floor() {
    // Treating it as empty would accept any old bundle: fail closed.
    let path = dir("floor-corrupt").join("floor.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(FileFloor::open(&path), Err(BundleError::Floor(_))));
}

#[test]
fn a_floor_that_cannot_be_written_refuses_the_bundle() {
    // Verify, then persist, then accept: a bundle it could forget is never handed back.
    let path = dir("floor-unwritable")
        .join("no-such-dir")
        .join("floor.json");
    let mut floor = FileFloor::open(&path).unwrap();
    let err = accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap_err();
    assert!(matches!(err, BundleError::Floor(_)), "{err:?}");
}

#[test]
fn the_write_is_atomic_and_leaves_no_temporary_file() {
    let d = dir("floor-atomic");
    let path = d.join("floor.json");
    let mut floor = FileFloor::open(&path).unwrap();
    accept(&bundle("uid-1", 3, t(1)), &keys(), t(2), &mut floor).unwrap();
    accept(&bundle("uid-1", 4, t(2)), &keys(), t(3), &mut floor).unwrap();
    let names: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["floor.json"]);
    let reopened = FileFloor::open(&path).unwrap();
    assert_eq!(reopened.get(NS, NAME).unwrap().version, 4);
}
