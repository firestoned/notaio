// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use chrono::TimeZone;
use serde_json::json;

use super::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0)
        .single()
        .unwrap()
}

fn ready(reason: &str, ok: bool) -> serde_json::Value {
    json!({
        "type": "Ready", "status": if ok { "True" } else { "False" }, "reason": reason,
        "message": "m", "lastTransitionTime": "2026-10-23T11:00:00Z"
    })
}

fn policy(name: &str, status: serde_json::Value) -> Arc<SandboxPolicy> {
    Arc::new(
        serde_json::from_value(json!({
            "apiVersion": "sandbox.firestoned.io/v1alpha1", "kind": "SandboxPolicy",
            "metadata": { "name": name, "namespace": "sandboxes" },
            "spec": { "profileRef": "hardened" },
            "status": status,
        }))
        .unwrap(),
    )
}

fn grant(name: &str, phase: &str, expires_at: &str) -> Arc<KnobGrant> {
    Arc::new(
        serde_json::from_value(json!({
            "apiVersion": "sandbox.firestoned.io/v1alpha1", "kind": "KnobGrant",
            "metadata": {
                "name": name, "namespace": "sandboxes",
                "creationTimestamp": "2026-10-23T10:00:00Z"
            },
            "spec": {
                "policyRef": "p", "knob": "K1.1", "step": "R1", "owner": "o", "reason": "r",
                "expiresAt": expires_at, "approvals": []
            },
            "status": { "phase": phase },
        }))
        .unwrap(),
    )
}

fn scrape(policies: &[Arc<SandboxPolicy>], grants: &[Arc<KnobGrant>], c: &Counters) -> String {
    render(
        c,
        &Snapshot {
            policies,
            profiles: &[],
            grants,
            now: now(),
            bundle_lifetime: Duration::hours(1),
        },
    )
}

fn has(out: &str, line: &str) {
    assert!(
        out.lines().any(|l| l == line),
        "missing {line:?} in:\n{out}"
    );
}

#[test]
fn the_must_be_zero_series_exist_when_nothing_happened() {
    // An alert on "must be 0" needs a series to compare, not an absence.
    let out = scrape(&[], &[], &Counters::default());
    has(&out, "notaio_relaxations_past_expiry 0");
    for check in FAIL_OPEN_CHECKS {
        has(
            &out,
            &format!("notaio_fail_open_total{{check=\"{check}\"}} 0"),
        );
    }
    has(&out, "notaio_bundles_published_total 0");
    assert!(out.ends_with("# EOF\n"), "{out}");
}

#[test]
fn policies_are_counted_by_ready_reason() {
    let out = scrape(
        &[
            policy("a", json!({ "conditions": [ready("Published", true)] })),
            policy("b", json!({ "conditions": [ready("Published", true)] })),
            policy(
                "c",
                json!({ "conditions": [ready("SigningFailed", false)] }),
            ),
            policy("d", json!({})),
        ],
        &[],
        &Counters::default(),
    );
    has(&out, "notaio_policies{reason=\"Published\"} 2");
    has(&out, "notaio_policies{reason=\"SigningFailed\"} 1");
    has(&out, "notaio_policies{reason=\"Pending\"} 1");
}

#[test]
fn relaxations_by_phase_age_and_expiry() {
    let out = scrape(
        &[],
        &[
            grant("live", "Active", "2026-10-23T13:00:00Z"),
            grant("old", "Expired", "2026-10-22T13:00:00Z"),
            grant("bad", "Rejected", "2026-10-23T13:00:00Z"),
        ],
        &Counters::default(),
    );
    has(&out, "notaio_relaxations{phase=\"Active\"} 1");
    has(&out, "notaio_relaxations{phase=\"Expired\"} 1");
    has(&out, "notaio_relaxations{phase=\"Pending\"} 0");
    has(
        &out,
        "notaio_relaxation_age_seconds{namespace=\"sandboxes\",grant=\"live\"} 7200.0",
    );
    has(
        &out,
        "notaio_relaxation_expires_in_seconds{namespace=\"sandboxes\",grant=\"live\"} 3600.0",
    );
    has(&out, "notaio_lint_failures{kind=\"KnobGrant\"} 1");
    // Only active grants get per-grant series.
    assert!(!out.contains("grant=\"old\""), "{out}");
    has(&out, "notaio_relaxations_past_expiry 0");
}

#[test]
fn an_active_grant_past_its_expiry_is_counted() {
    let out = scrape(
        &[],
        &[grant("late", "Active", "2026-10-23T11:59:00Z")],
        &Counters::default(),
    );
    has(&out, "notaio_relaxations_past_expiry 1");
}

#[test]
fn bundle_age_follows_from_not_after_and_the_lifetime() {
    let out = scrape(
        &[policy(
            "p",
            json!({
                "conditions": [ready("Published", true)],
                "bundleVersion": 4,
                "bundleNotAfter": "2026-10-23T12:40:00Z",
            }),
        )],
        &[],
        &Counters::default(),
    );
    // Issued at 11:40 for an hour: 20 minutes old, 40 to go.
    has(
        &out,
        "notaio_bundle_age_seconds{namespace=\"sandboxes\",policy=\"p\"} 1200.0",
    );
    has(
        &out,
        "notaio_bundle_expires_in_seconds{namespace=\"sandboxes\",policy=\"p\"} 2400.0",
    );
}

#[test]
fn counters_accumulate_across_scrapes() {
    let c = Counters::default();
    c.bundles_published.inc();
    c.signing_failures.inc();
    c.fail_open("no-ceiling");
    let _ = scrape(&[], &[], &c);
    c.bundles_published.inc();
    let out = scrape(&[], &[], &c);
    has(&out, "notaio_bundles_published_total 2");
    has(&out, "notaio_signing_failures_total 1");
    has(&out, "notaio_fail_open_total{check=\"no-ceiling\"} 1");
    has(&out, "notaio_fail_open_total{check=\"already-expired\"} 0");
}
