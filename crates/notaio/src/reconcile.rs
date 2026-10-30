// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{Duration, Utc};
use k8s_openapi::api::core::v1::{ConfigMap, ObjectReference};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, ListParams, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::runtime::events::{Event, EventType, Recorder};
use kube::{Client, Resource, ResourceExt};
use notaio::events::{self, Note};
use notaio::guard::{self, Evaluated};
use notaio::metrics::Counters;
use sandboxpolicy_api::{
    ExposureCeiling, GrantPhase, KnobGrant, KnobGrantStatus, SandboxPolicy, SandboxPolicyStatus,
    SandboxProfile, SandboxProfileStatus,
};
use sandboxpolicy_core::ceilings::Ceilings;
use sandboxpolicy_core::evaluate::{condition, profile_digest};
use sandboxpolicy_core::lint::{has_errors, lint_knobs, summarize};
use sandboxpolicy_core::{evaluate, EvalInput};
use serde_json::json;

use crate::error::Error;
use crate::signer::SharedSigner;

pub const FIELD_MANAGER: &str = "notaio";
pub const ANN_VERSION: &str = "sandbox.firestoned.io/bundle-version";
pub const ANN_DIGEST: &str = "sandbox.firestoned.io/bundle-digest";
pub const BUNDLE_KEY: &str = "bundle.dsse.json";

pub struct Ctx {
    pub client: Client,
    pub signer: Option<SharedSigner>,
    pub ceilings: Ceilings,
    /// How long a compiled bundle stays valid. The upper bound on staleness.
    pub bundle_lifetime: Duration,
    pub counters: Counters,
    pub recorder: Recorder,
    /// True while this replica holds the leader Lease (ADR-0011).
    pub leading: Arc<std::sync::atomic::AtomicBool>,
}

impl Ctx {
    fn leading(&self) -> bool {
        self.leading.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// Record `note` on `regarding`, if there is one. A failure is logged, never returned: an Event is
/// a report, and must not hold back a reconcile (ADR-0010, decision 5).
async fn record(ctx: &Ctx, regarding: &ObjectReference, note: Option<Note>) {
    let Some(n) = note else { return };
    let ev = Event {
        type_: if n.warning {
            EventType::Warning
        } else {
            EventType::Normal
        },
        reason: n.reason,
        note: Some(n.note),
        action: events::ACTION.to_string(),
        secondary: None,
    };
    if let Err(e) = ctx.recorder.publish(&ev, regarding).await {
        tracing::warn!(error = %e, object = ?regarding.name, "could not record an Event");
    }
}

/// `status` with its Ready condition replaced by a false one with `reason` and `message`.
fn not_ready(
    prev: &SandboxPolicyStatus,
    status: &SandboxPolicyStatus,
    reason: &str,
    message: &str,
    now: chrono::DateTime<Utc>,
    generation: Option<i64>,
) -> SandboxPolicyStatus {
    let failed = condition(
        &prev.conditions,
        "Ready",
        false,
        reason,
        message,
        now,
        generation,
    );
    let mut out = status.clone();
    if let Some(c) = out.conditions.iter_mut().find(|c| c.type_ == "Ready") {
        *c = failed;
    }
    out
}

pub fn bundle_object_name(policy: &str) -> String {
    format!("{policy}-bundle")
}

fn std_dur(d: Duration) -> StdDuration {
    d.to_std().unwrap_or(StdDuration::from_secs(30))
}

/// Same status, ignoring the refreshable expiry timestamp.
fn same_modulo_expiry(a: &SandboxPolicyStatus, b: &SandboxPolicyStatus) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.bundle_not_after = None;
    b.bundle_not_after = None;
    a == b
}

pub async fn reconcile_policy(policy: Arc<SandboxPolicy>, ctx: Arc<Ctx>) -> Result<Action, Error> {
    // A replica that has lost the Lease writes nothing; it is about to exit (ADR-0011).
    if !ctx.leading() {
        return Ok(Action::await_change());
    }
    let ns = policy.namespace().ok_or(Error::NoNamespace)?;
    let name = policy.name_any();
    let client = &ctx.client;
    let now = Utc::now();

    let profile = Api::<SandboxProfile>::all(client.clone())
        .get_opt(&policy.spec.profile_ref)
        .await?;
    let grants_api: Api<KnobGrant> = Api::namespaced(client.clone(), &ns);
    let grants = grants_api.list(&ListParams::default()).await?.items;
    // The approved maximum exposure has the policy's own name (ADR-0007). None fails closed.
    let exposure_ceiling = Api::<ExposureCeiling>::namespaced(client.clone(), &ns)
        .get_opt(&name)
        .await?;

    // The version must never go backwards, even if status was lost, so the published object is a
    // second source of truth.
    let cms: Api<ConfigMap> = Api::namespaced(client.clone(), &ns);
    let cm_name = bundle_object_name(&name);
    let published = cms.get_opt(&cm_name).await?;
    let ann = |key: &str| {
        published
            .as_ref()
            .and_then(|c| c.metadata.annotations.as_ref())
            .and_then(|a| a.get(key).cloned())
    };
    let cm_version: Option<u64> = ann(ANN_VERSION).and_then(|v| v.parse().ok());
    let cm_digest = ann(ANN_DIGEST);

    let prev = policy.status.clone().unwrap_or_default();
    let (prev_version, prev_digest) = match (prev.bundle_version, cm_version) {
        (Some(s), Some(c)) if c > s => (Some(c), cm_digest),
        (Some(s), _) => (Some(s), prev.bundle_digest.clone()),
        (None, Some(c)) => (Some(c), cm_digest),
        (None, None) => (None, None),
    };

    let out = evaluate(&EvalInput {
        now,
        exposure_ceiling: exposure_ceiling.as_ref(),
        policy: &policy,
        profile: profile.as_ref(),
        grants: &grants,
        prev_version,
        prev_digest: prev_digest.as_deref(),
        prev_conditions: &prev.conditions,
        bundle_lifetime: ctx.bundle_lifetime,
        ceilings: &ctx.ceilings,
    });
    for f in &out.findings {
        tracing::debug!(policy = %name, code = f.code, path = %f.path, "{}", f.message);
    }

    // Grant statuses: one writer, and only when something changed.
    for r in &out.grants {
        let Some(g) = grants.iter().find(|g| g.name_any() == r.name) else {
            continue;
        };
        let old = g.status.clone().unwrap_or_default();
        let ok = r.phase == GrantPhase::Active;
        let cond = condition(
            &old.conditions,
            "Applied",
            ok,
            &format!("{:?}", r.phase),
            &r.message,
            now,
            g.metadata.generation,
        );
        if old.phase != r.phase {
            record(
                &ctx,
                &g.object_ref(&()),
                events::grant(old.phase, r.phase, &r.message),
            )
            .await;
        }
        if old.phase != r.phase
            || old.observed_generation != g.metadata.generation
            || old.conditions.first().map(|c| c.message.as_str()) != Some(r.message.as_str())
        {
            let status = KnobGrantStatus {
                phase: r.phase,
                conditions: vec![cond],
                observed_generation: g.metadata.generation,
            };
            grants_api
                .patch_status(
                    &r.name,
                    &PatchParams::default(),
                    &Patch::Merge(json!({ "status": status })),
                )
                .await?;
        }
    }

    // Readiness means compiled, signed and published. Decide it before comparing with old status so
    // an unchanged policy produces an identical status and no write.
    let mut status = out.status.clone();
    if out.bundle.is_some() {
        let ready = match &ctx.signer {
            Some(_) => condition(
                &prev.conditions,
                "Ready",
                true,
                "Published",
                "signed bundle published",
                now,
                policy.metadata.generation,
            ),
            None => condition(
                &prev.conditions,
                "Ready",
                false,
                "NoSigner",
                "no signing key configured: bundle not published",
                now,
                policy.metadata.generation,
            ),
        };
        if let Some(c) = status.conditions.iter_mut().find(|c| c.type_ == "Ready") {
            *c = ready;
        }
    }

    let stale = prev
        .bundle_not_after
        .is_none_or(|t| t <= now + ctx.bundle_lifetime / 2);
    let unchanged = same_modulo_expiry(&prev, &status)
        && !(stale && status.bundle_not_after != prev.bundle_not_after);
    if unchanged {
        return Ok(Action::requeue(std_dur(out.requeue_after)));
    }

    if let (Some(bundle), Some(signer)) = (&out.bundle, &ctx.signer) {
        // Check the bundle again, independently of the evaluator, before it can be signed
        // (ADR-0010, decision 4). In a correct build this never refuses anything.
        let active_grant_expiries: Vec<(String, chrono::DateTime<Utc>)> = out
            .status
            .active_grants
            .iter()
            .filter_map(|n| {
                grants
                    .iter()
                    .find(|g| g.name_any() == *n)
                    .map(|g| (n.clone(), g.spec.expires_at))
            })
            .collect();
        let failed_checks = guard::check(
            &Evaluated {
                not_after: bundle.meta.not_after,
                has_ceiling: exposure_ceiling.is_some(),
                findings: &out.findings,
                active_grant_expiries: &active_grant_expiries,
            },
            now,
        );
        if !failed_checks.is_empty() {
            for c in &failed_checks {
                ctx.counters.fail_open(c);
            }
            let msg = format!(
                "refused to publish: the bundle failed the fail-open checks {}",
                failed_checks.join(", ")
            );
            tracing::error!(policy = %name, checks = ?failed_checks, "{msg}");
            return refuse(&ctx, &policy, &prev, &status, "FailOpenRefused", &msg, now)
                .await
                .map(|()| Action::requeue(StdDuration::from_secs(300)));
        }

        if !ctx.leading() {
            return Ok(Action::await_change());
        }
        let envelope = match signer.sign(bundle).await {
            Ok(e) => e,
            Err(e) => {
                // Fail closed: nothing new is published. Say why on the policy, and keep the
                // previous bundle's version and digest, which are still what consumers hold.
                ctx.counters.signing_failures.inc();
                refuse(
                    &ctx,
                    &policy,
                    &prev,
                    &status,
                    "SigningFailed",
                    &e.to_string(),
                    now,
                )
                .await?;
                return Err(e);
            }
        };
        let mut annotations = BTreeMap::new();
        annotations.insert(ANN_VERSION.to_string(), bundle.meta.version.to_string());
        annotations.insert(ANN_DIGEST.to_string(), bundle.content_digest.clone());
        let mut labels = BTreeMap::new();
        labels.insert(
            "app.kubernetes.io/managed-by".to_string(),
            FIELD_MANAGER.to_string(),
        );
        let cm = ConfigMap {
            metadata: ObjectMeta {
                name: Some(cm_name.clone()),
                namespace: Some(ns.clone()),
                labels: Some(labels),
                annotations: Some(annotations),
                owner_references: policy.controller_owner_ref(&()).map(|o| vec![o]),
                ..Default::default()
            },
            data: Some(BTreeMap::from([(
                BUNDLE_KEY.to_string(),
                serde_json::to_string(&envelope)?,
            )])),
            ..Default::default()
        };
        cms.patch(
            &cm_name,
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(&cm),
        )
        .await?;
        ctx.counters.bundles_published.inc();
    }

    Api::<SandboxPolicy>::namespaced(client.clone(), &ns)
        .patch_status(
            &name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": status })),
        )
        .await?;
    record(
        &ctx,
        &policy.object_ref(&()),
        events::policy(&prev, &status),
    )
    .await;
    Ok(Action::requeue(std_dur(out.requeue_after)))
}

/// Publish nothing: set Ready false with `reason` on the policy, keeping the previous bundle's
/// version and digest, which are still what consumers hold, and record the transition.
async fn refuse(
    ctx: &Ctx,
    policy: &SandboxPolicy,
    prev: &SandboxPolicyStatus,
    status: &SandboxPolicyStatus,
    reason: &str,
    message: &str,
    now: chrono::DateTime<Utc>,
) -> Result<(), Error> {
    let ns = policy.namespace().ok_or(Error::NoNamespace)?;
    let refused = not_ready(
        prev,
        status,
        reason,
        message,
        now,
        policy.metadata.generation,
    );
    Api::<SandboxPolicy>::namespaced(ctx.client.clone(), &ns)
        .patch_status(
            &policy.name_any(),
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": { "conditions": refused.conditions } })),
        )
        .await?;
    // Compared with the conditions only: the version and digest did not move.
    let before = SandboxPolicyStatus {
        conditions: prev.conditions.clone(),
        ..Default::default()
    };
    let after = SandboxPolicyStatus {
        conditions: refused.conditions,
        ..Default::default()
    };
    record(
        ctx,
        &policy.object_ref(&()),
        events::policy(&before, &after),
    )
    .await;
    Ok(())
}

pub async fn reconcile_profile(
    profile: Arc<SandboxProfile>,
    ctx: Arc<Ctx>,
) -> Result<Action, Error> {
    if !ctx.leading() {
        return Ok(Action::await_change());
    }
    let now = Utc::now();
    let name = profile.name_any();
    let prev = profile.status.clone().unwrap_or_default();
    let findings = lint_knobs(profile.spec.data_scope, &profile.spec.knobs);
    let ok = !has_errors(&findings);
    let cond = condition(
        &prev.conditions,
        "Valid",
        ok,
        if ok { "Valid" } else { "Invalid" },
        &if ok {
            "profile passes every lint rule".to_string()
        } else {
            summarize(&findings)
        },
        now,
        profile.metadata.generation,
    );
    let status = SandboxProfileStatus {
        conditions: vec![cond],
        observed_generation: profile.metadata.generation,
        digest: ok.then(|| profile_digest(&profile.spec)),
    };
    if status != prev {
        record(
            &ctx,
            &profile.object_ref(&()),
            events::profile(&prev.conditions, &status.conditions),
        )
        .await;
        Api::<SandboxProfile>::all(ctx.client.clone())
            .patch_status(
                &name,
                &PatchParams::default(),
                &Patch::Merge(json!({ "status": status })),
            )
            .await?;
    }
    Ok(Action::await_change())
}

pub fn error_policy<K>(_obj: Arc<K>, err: &Error, ctx: Arc<Ctx>) -> Action {
    ctx.counters.reconcile_errors.inc();
    tracing::warn!(error = %err, "reconcile failed, retrying");
    Action::requeue(StdDuration::from_secs(30))
}
