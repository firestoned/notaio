// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use chrono::{Duration, TimeZone};
use sandboxpolicy_core::lint::{Finding, Severity};

use super::*;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0)
        .single()
        .unwrap()
}

fn finding(severity: Severity) -> Finding {
    Finding {
        code: "L001",
        severity,
        path: "spec.knobs".to_string(),
        message: "test".to_string(),
    }
}

fn good() -> Evaluated<'static> {
    Evaluated {
        not_after: now() + Duration::hours(1),
        has_ceiling: true,
        findings: &[],
        active_grant_expiries: &[],
    }
}

#[test]
fn a_sound_evaluation_passes() {
    assert!(check(&good(), now()).is_empty());
}

#[test]
fn warnings_and_live_grants_pass() {
    let findings = [finding(Severity::Warning)];
    let grants = [("g".to_string(), now() + Duration::minutes(5))];
    let e = Evaluated {
        findings: &findings,
        active_grant_expiries: &grants,
        ..good()
    };
    assert!(check(&e, now()).is_empty());
}

#[test]
fn each_check_fails_on_its_own() {
    let errors = [finding(Severity::Error)];
    // Expiring exactly now counts as expired, as it does for the evaluator and the consumers.
    let lapsed = [("g".to_string(), now())];
    let cases: [(Evaluated<'_>, &str); 4] = [
        (
            Evaluated {
                has_ceiling: false,
                ..good()
            },
            "no-ceiling",
        ),
        (
            Evaluated {
                findings: &errors,
                ..good()
            },
            "error-findings",
        ),
        (
            Evaluated {
                active_grant_expiries: &lapsed,
                ..good()
            },
            "expired-relaxation",
        ),
        (
            Evaluated {
                not_after: now(),
                ..good()
            },
            "already-expired",
        ),
    ];
    for (e, want) in cases {
        assert_eq!(check(&e, now()), vec![want]);
    }
}

#[test]
fn every_check_name_is_a_metric_label() {
    // A check the metric does not pre-register would be absent rather than 0 until it fired.
    let errors = [finding(Severity::Error)];
    let lapsed = [("g".to_string(), now())];
    let all = Evaluated {
        not_after: now(),
        has_ceiling: false,
        findings: &errors,
        active_grant_expiries: &lapsed,
    };
    assert_eq!(
        check(&all, now()),
        crate::metrics::FAIL_OPEN_CHECKS.to_vec()
    );
}
