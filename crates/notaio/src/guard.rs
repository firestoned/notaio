// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The fail-open guard (ADR-0010, decision 4).
//!
//! Before a bundle is signed, the controller checks it again, independently of the evaluator in
//! `sandboxpolicy-core`. Each check is something the evaluator already guarantees, so in a correct
//! build none of them ever fails. If one does, nothing is published and
//! `notaio_fail_open_total{check}` counts it.

use chrono::{DateTime, Utc};
use sandboxpolicy_core::lint::{Finding, Severity};

/// What the guard needs to know about an evaluation that produced a bundle.
pub struct Evaluated<'a> {
    /// The bundle's `meta.notAfter`.
    pub not_after: DateTime<Utc>,
    /// Whether an `ExposureCeiling` for the policy existed (ADR-0007).
    pub has_ceiling: bool,
    pub findings: &'a [Finding],
    /// The expiry of every grant the bundle applies, by grant name.
    pub active_grant_expiries: &'a [(String, DateTime<Utc>)],
}

/// The checks `e` fails, by their stable names in `crate::metrics::FAIL_OPEN_CHECKS`. Empty means
/// the bundle may be signed.
pub fn check(e: &Evaluated<'_>, now: DateTime<Utc>) -> Vec<&'static str> {
    let mut failed = Vec::new();
    if !e.has_ceiling {
        failed.push("no-ceiling");
    }
    if e.findings.iter().any(|f| f.severity == Severity::Error) {
        failed.push("error-findings");
    }
    if e.active_grant_expiries.iter().any(|(_, t)| *t <= now) {
        failed.push("expired-relaxation");
    }
    if e.not_after <= now {
        failed.push("already-expired");
    }
    failed
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
