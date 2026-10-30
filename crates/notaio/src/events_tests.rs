// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use chrono::{TimeZone, Utc};
use sandboxpolicy_core::evaluate::condition;

use super::*;

fn cond(type_: &str, ok: bool, reason: &str, message: &str) -> Condition {
    let now = Utc
        .with_ymd_and_hms(2026, 10, 23, 12, 0, 0)
        .single()
        .unwrap();
    condition(&[], type_, ok, reason, message, now, Some(1))
}

fn status(ready: Condition, version: Option<u64>) -> SandboxPolicyStatus {
    SandboxPolicyStatus {
        conditions: vec![ready],
        bundle_version: version,
        ..Default::default()
    }
}

fn published(version: u64) -> SandboxPolicyStatus {
    status(
        cond("Ready", true, "Published", "signed bundle published"),
        Some(version),
    )
}

#[test]
fn a_new_bundle_version_is_published() {
    let n = policy(&SandboxPolicyStatus::default(), &published(1)).unwrap();
    assert!(!n.warning);
    assert_eq!(n.reason, "Published");
    assert!(n.note.contains("version 1"), "{}", n.note);
    assert_eq!(
        policy(&published(1), &published(2)).unwrap().reason,
        "Published"
    );
}

#[test]
fn an_unchanged_or_refreshed_bundle_records_nothing() {
    // The half-life refresh rewrites notAfter but keeps the version: no Event every half hour.
    assert_eq!(policy(&published(3), &published(3)), None);
}

#[test]
fn a_failure_is_a_warning_with_its_reason() {
    let failed = status(
        cond("Ready", false, "SigningFailed", "OpenBao is sealed"),
        Some(3),
    );
    let n = policy(&published(3), &failed).unwrap();
    assert!(n.warning);
    assert_eq!(n.reason, "SigningFailed");
    assert_eq!(n.note, "OpenBao is sealed");
}

#[test]
fn the_same_failure_again_records_nothing_a_new_one_does() {
    let a = status(cond("Ready", false, "NoApprovedCeiling", "m"), None);
    assert_eq!(policy(&a, &a), None);
    let b = status(cond("Ready", false, "InvalidPolicy", "m"), None);
    assert_eq!(policy(&a, &b).unwrap().reason, "InvalidPolicy");
}

#[test]
fn no_ready_condition_records_nothing() {
    assert_eq!(policy(&published(1), &SandboxPolicyStatus::default()), None);
}

#[test]
fn grants_record_lapses_and_refusals_only() {
    use GrantPhase::*;
    let lapsed = grant(Active, Expired, "expired at 12:00").unwrap();
    assert!(!lapsed.warning);
    assert_eq!(lapsed.reason, "Expired");
    let refused = grant(Pending, Rejected, "L010: no approval").unwrap();
    assert!(refused.warning);
    assert_eq!(refused.reason, "Rejected");
    assert_eq!(grant(Pending, Active, "applied"), None);
    assert_eq!(grant(Expired, Expired, "still expired"), None);
}

#[test]
fn profiles_record_becoming_invalid_only() {
    let valid = [cond("Valid", true, "Valid", "ok")];
    let invalid = [cond("Valid", false, "Invalid", "L003: too wide")];
    let n = profile(&valid, &invalid).unwrap();
    assert!(n.warning);
    assert_eq!(n.reason, "Invalid");
    assert_eq!(profile(&invalid, &invalid), None);
    assert_eq!(profile(&invalid, &valid), None);
    let worse = [cond("Valid", false, "Invalid", "L003, L004")];
    assert!(profile(&invalid, &worse).is_some());
}

#[test]
fn notes_fit_the_api_servers_limit() {
    assert_eq!(fit("short"), "short");
    // Multi-byte characters straddling the limit are not split.
    let long = "é".repeat(MAX_NOTE);
    let cut = fit(&long);
    assert!(cut.len() <= MAX_NOTE, "{}", cut.len());
    assert!(cut.ends_with(" [truncated]"));
    let failed = status(cond("Ready", false, "SigningFailed", &long), None);
    assert!(
        policy(&SandboxPolicyStatus::default(), &failed)
            .unwrap()
            .note
            .len()
            <= MAX_NOTE
    );
}
