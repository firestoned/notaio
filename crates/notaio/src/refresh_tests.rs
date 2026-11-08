// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use chrono::TimeZone;

fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + secs, 0).unwrap()
}

const LIFETIME: i64 = 900;

#[test]
fn no_bundle_is_stale() {
    assert!(is_stale(None, Duration::seconds(LIFETIME), at(0)));
}

#[test]
fn stale_from_half_the_lifetime_left() {
    let lifetime = Duration::seconds(LIFETIME);
    let not_after = at(LIFETIME);
    assert!(!is_stale(Some(not_after), lifetime, at(449)));
    assert!(is_stale(Some(not_after), lifetime, at(450)));
    assert!(is_stale(Some(not_after), lifetime, at(LIFETIME + 1)));
}

#[test]
fn the_core_incident_rechecks_when_the_bundle_turns_stale() {
    // 2026-11-08 on the test cluster: an hour bundle with 655 s left, a 900 s lifetime. The
    // controller waited the evaluator's 450 s and re-signed with 205 s left, so
    // NotaioBundleNearExpiry fired. The next check must come when 450 s are left.
    let lifetime = Duration::seconds(LIFETIME);
    let now = at(0);
    let not_after = at(655);
    let wait = next_check(Duration::seconds(450), Some(not_after), lifetime, now);
    assert!(wait < Duration::seconds(450), "waited {wait}");
    assert!(is_stale(Some(not_after), lifetime, now + wait));
}

#[test]
fn every_phase_refreshes_by_half_the_lifetime_plus_slack() {
    // Whatever moment the last unchanged reconcile ran at, the next one finds the bundle stale
    // with at least half the lifetime minus the slack left (ADR-0018, decision 1).
    let lifetime = Duration::seconds(LIFETIME);
    let requeue = lifetime / 2;
    for left in (LIFETIME / 2 + 1)..=LIFETIME * 4 {
        let now = at(0);
        let not_after = at(left);
        let wait = next_check(requeue, Some(not_after), lifetime, now);
        let then = now + wait;
        if !is_stale(Some(not_after), lifetime, then) {
            // Not stale yet: only allowed when the evaluator's own requeue came first.
            assert_eq!(wait, requeue, "left {left}");
            continue;
        }
        let remaining = not_after - then;
        assert!(
            remaining >= lifetime / 2 - RECHECK_SLACK,
            "left {left}: re-signed with {remaining} left"
        );
    }
}

#[test]
fn a_just_signed_bundle_keeps_the_evaluators_requeue() {
    let lifetime = Duration::seconds(LIFETIME);
    let wait = next_check(lifetime / 2, Some(at(LIFETIME)), lifetime, at(0));
    assert_eq!(wait, lifetime / 2);
}

#[test]
fn an_earlier_grant_expiry_still_wins() {
    let lifetime = Duration::seconds(LIFETIME);
    let wait = next_check(Duration::seconds(30), Some(at(700)), lifetime, at(0));
    assert_eq!(wait, Duration::seconds(30));
}

#[test]
fn no_bundle_keeps_the_evaluators_requeue() {
    let lifetime = Duration::seconds(LIFETIME);
    let wait = next_check(Duration::seconds(450), None, lifetime, at(0));
    assert_eq!(wait, Duration::seconds(450));
}

#[test]
fn an_already_stale_bundle_is_rechecked_soon_not_in_a_hot_loop() {
    let lifetime = Duration::seconds(LIFETIME);
    let wait = next_check(Duration::seconds(450), Some(at(100)), lifetime, at(0));
    assert_eq!(wait, MIN_RECHECK);
}
