// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Prometheus metrics (ADR-0010, decision 4).
//!
//! Gauges are computed at scrape time from the controller's caches, so a deleted object leaves no
//! stale series. Counters count since start. `render` is a function of its inputs, and `now` comes
//! in as an argument, so a test can pin every value.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use sandboxpolicy_api::{Condition, GrantPhase, KnobGrant, SandboxPolicy, SandboxProfile};

/// The content type of `render`'s output.
pub const CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

type Labels = Vec<(String, String)>;

/// The fail-open checks (`crate::guard`), each a stable label value of `notaio_fail_open_total`.
pub const FAIL_OPEN_CHECKS: [&str; 4] = [
    "no-ceiling",
    "error-findings",
    "expired-relaxation",
    "already-expired",
];

/// Counters the controller increments as things happen. Cloning shares the values.
#[derive(Clone, Default)]
pub struct Counters {
    pub bundles_published: Counter,
    pub signing_failures: Counter,
    pub reconcile_errors: Counter,
    /// 1 on the replica holding the leader Lease, 0 on the others (ADR-0011).
    pub leader: Gauge,
    fail_open: Family<Labels, Counter>,
}

impl Counters {
    /// Count one refused publication for `check`, one of `FAIL_OPEN_CHECKS`.
    pub fn fail_open(&self, check: &str) {
        self.fail_open
            .get_or_create(&vec![("check".to_string(), check.to_string())])
            .inc();
    }
}

/// What the gauges are computed from: the controller's caches at scrape time.
pub struct Snapshot<'a> {
    pub policies: &'a [std::sync::Arc<SandboxPolicy>],
    pub profiles: &'a [std::sync::Arc<SandboxProfile>],
    pub grants: &'a [std::sync::Arc<KnobGrant>],
    pub now: DateTime<Utc>,
    /// The configured bundle lifetime, so a bundle's age follows from its `notAfter`.
    pub bundle_lifetime: Duration,
}

fn find<'c>(conditions: &'c [Condition], type_: &str) -> Option<&'c Condition> {
    conditions.iter().find(|c| c.type_ == type_)
}

fn is_false(conditions: &[Condition], type_: &str) -> bool {
    find(conditions, type_).is_some_and(|c| c.status == "False")
}

fn label(pairs: &[(&str, &str)]) -> Labels {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn seconds(d: Duration) -> f64 {
    // Millisecond precision is plenty for ages and expiries, and keeps the cast exact.
    d.num_milliseconds() as f64 / 1000.0
}

fn phase_name(p: GrantPhase) -> &'static str {
    match p {
        GrantPhase::Pending => "Pending",
        GrantPhase::Active => "Active",
        GrantPhase::Rejected => "Rejected",
        GrantPhase::Expired => "Expired",
    }
}

/// The text exposition of every metric, for `GET /metrics`.
pub fn render(counters: &Counters, snap: &Snapshot<'_>) -> String {
    let mut registry = Registry::default();
    let now = snap.now;

    let policies = Family::<Labels, Gauge>::default();
    let mut by_reason: BTreeMap<String, i64> = BTreeMap::new();
    for p in snap.policies {
        let conditions = p
            .status
            .as_ref()
            .map(|s| s.conditions.as_slice())
            .unwrap_or_default();
        let reason = find(conditions, "Ready").map_or("Pending", |c| c.reason.as_str());
        *by_reason.entry(reason.to_string()).or_default() += 1;
    }
    for (reason, n) in &by_reason {
        policies
            .get_or_create(&label(&[("reason", reason)]))
            .set(*n);
    }
    registry.register(
        "notaio_policies",
        "Sandbox policies by the reason of their Ready condition",
        policies,
    );

    let lint = Family::<Labels, Gauge>::default();
    let failing =
        |conditions: Option<&[Condition]>| conditions.is_some_and(|c| is_false(c, "Valid"));
    let profiles_failing = snap
        .profiles
        .iter()
        .filter(|p| failing(p.status.as_ref().map(|s| s.conditions.as_slice())))
        .count();
    let policies_failing = snap
        .policies
        .iter()
        .filter(|p| failing(p.status.as_ref().map(|s| s.conditions.as_slice())))
        .count();
    let grants_rejected = snap
        .grants
        .iter()
        .filter(|g| {
            g.status
                .as_ref()
                .is_some_and(|s| s.phase == GrantPhase::Rejected)
        })
        .count();
    for (kind, n) in [
        ("SandboxProfile", profiles_failing),
        ("SandboxPolicy", policies_failing),
        ("KnobGrant", grants_rejected),
    ] {
        lint.get_or_create(&label(&[("kind", kind)]))
            .set(i64::try_from(n).unwrap_or(i64::MAX));
    }
    registry.register(
        "notaio_lint_failures",
        "Objects failing lint now: profiles and policies with Valid false, rejected grants",
        lint,
    );

    let relaxations = Family::<Labels, Gauge>::default();
    let age = Family::<Labels, Gauge<f64, std::sync::atomic::AtomicU64>>::default();
    let expires_in = Family::<Labels, Gauge<f64, std::sync::atomic::AtomicU64>>::default();
    let past_expiry: Gauge = Gauge::default();
    for phase in [
        GrantPhase::Pending,
        GrantPhase::Active,
        GrantPhase::Rejected,
        GrantPhase::Expired,
    ] {
        let _ = relaxations.get_or_create(&label(&[("phase", phase_name(phase))]));
    }
    for g in snap.grants {
        let phase = g.status.as_ref().map(|s| s.phase).unwrap_or_default();
        relaxations
            .get_or_create(&label(&[("phase", phase_name(phase))]))
            .inc();
        if phase != GrantPhase::Active {
            continue;
        }
        let ns = g.metadata.namespace.as_deref().unwrap_or_default();
        let name = g.metadata.name.as_deref().unwrap_or_default();
        let key = label(&[("namespace", ns), ("grant", name)]);
        if let Some(created) = g
            .metadata
            .creation_timestamp
            .as_ref()
            .and_then(|t| DateTime::<Utc>::from_timestamp_millis(t.0.as_millisecond()))
        {
            age.get_or_create(&key).set(seconds(now - created));
        }
        expires_in
            .get_or_create(&key)
            .set(seconds(g.spec.expires_at - now));
        if g.spec.expires_at <= now {
            past_expiry.inc();
        }
    }
    registry.register(
        "notaio_relaxations",
        "Knob grants (relaxations) by phase",
        relaxations,
    );
    registry.register(
        "notaio_relaxation_age_seconds",
        "Age of each active relaxation",
        age,
    );
    registry.register(
        "notaio_relaxation_expires_in_seconds",
        "Time until each active relaxation expires",
        expires_in,
    );
    registry.register(
        "notaio_relaxations_past_expiry",
        "Active relaxations whose expiresAt has passed. Must be 0",
        past_expiry,
    );

    let bundle_age = Family::<Labels, Gauge<f64, std::sync::atomic::AtomicU64>>::default();
    let bundle_expires_in = Family::<Labels, Gauge<f64, std::sync::atomic::AtomicU64>>::default();
    for p in snap.policies {
        let Some(status) = &p.status else { continue };
        let (Some(_), Some(not_after)) = (status.bundle_version, status.bundle_not_after) else {
            continue;
        };
        let ns = p.metadata.namespace.as_deref().unwrap_or_default();
        let name = p.metadata.name.as_deref().unwrap_or_default();
        let key = label(&[("namespace", ns), ("policy", name)]);
        bundle_age
            .get_or_create(&key)
            .set(seconds(now - (not_after - snap.bundle_lifetime)));
        bundle_expires_in
            .get_or_create(&key)
            .set(seconds(not_after - now));
    }
    registry.register(
        "notaio_bundle_age_seconds",
        "Age of each policy's published bundle",
        bundle_age,
    );
    registry.register(
        "notaio_bundle_expires_in_seconds",
        "Time until each policy's published bundle passes notAfter",
        bundle_expires_in,
    );

    registry.register(
        "notaio_leader",
        "1 on the replica holding the leader Lease, 0 on the others",
        counters.leader.clone(),
    );
    registry.register(
        "notaio_bundles_published",
        "Bundles signed and published",
        counters.bundles_published.clone(),
    );
    registry.register(
        "notaio_signing_failures",
        "Signing attempts that failed, so nothing was published",
        counters.signing_failures.clone(),
    );
    registry.register(
        "notaio_reconcile_errors",
        "Reconciles that returned an error and were retried",
        counters.reconcile_errors.clone(),
    );
    // Every check is present from the start, so "must be 0" is a series, not an absence.
    for check in FAIL_OPEN_CHECKS {
        let _ = counters
            .fail_open
            .get_or_create(&label(&[("check", check)]));
    }
    registry.register(
        "notaio_fail_open",
        "Publications refused because the bundle failed a check the evaluator should have made. Must be 0",
        counters.fail_open.clone(),
    );

    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = encode(&mut out, &registry);
    out
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
