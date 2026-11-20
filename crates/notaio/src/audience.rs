// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The `SandboxAudience` reconcile (ADR-0020): lint the definition against every other audience,
//! compile it, sign it with the policy bundles' key and publish it to `<audience>-audience` in the
//! audience's namespace. Runs only when the platform audience namespace is configured.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use jiff::Timestamp;
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::runtime::reflector::Store;
use kube::{Resource, ResourceExt};
use notaio::events;
use notaio::guard::{self, Evaluated};
use notaio::refresh;
use sandboxpolicy_api::{SandboxAudience, SandboxAudienceStatus};
use sandboxpolicy_core::evaluate::condition;
use sandboxpolicy_core::{evaluate_audience, AudienceCatalog, AudienceEvalInput};
use serde_json::json;

use crate::error::Error;
use crate::reconcile::{record, std_dur, Ctx, ANN_DIGEST, ANN_VERSION, FIELD_MANAGER};

/// The key of the envelope in the audience's ConfigMap (ADR-0020, decision 10).
pub const AUDIENCE_BUNDLE_KEY: &str = "audience.dsse.json";

/// How long to wait before trying a refused publication again.
const REFUSED_REQUEUE: StdDuration = StdDuration::from_secs(300);

/// The ConfigMap an audience's bundle is published to. It cannot be a policy's `<policy>-bundle`:
/// the suffixes differ.
pub fn audience_object_name(audience: &str) -> String {
    format!("{audience}-audience")
}

/// What the audience reconcile needs beyond the shared [`Ctx`].
pub struct AudienceCtx {
    pub ctx: Arc<Ctx>,
    /// The platform audience namespace (ADR-0020, decision 2).
    pub platform_namespace: String,
    /// Every `SandboxAudience`, filled before the controllers start.
    pub audiences: Store<SandboxAudience>,
}

/// Every audience the store holds, with `current` in place of its cached copy: the reconcile's own
/// object may be newer than the cache.
pub fn snapshot(
    store: &Store<SandboxAudience>,
    current: Option<&SandboxAudience>,
) -> Vec<SandboxAudience> {
    let key = |a: &SandboxAudience| (a.namespace(), a.name_any());
    let mut out: Vec<SandboxAudience> = store
        .state()
        .iter()
        .map(|a| (**a).clone())
        .filter(|a| current.is_none_or(|c| key(c) != key(a)))
        .collect();
    out.extend(current.cloned());
    out
}

/// Same status, ignoring the refreshable expiry timestamp.
fn same_modulo_expiry(a: &SandboxAudienceStatus, b: &SandboxAudienceStatus) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.bundle_not_after = None;
    b.bundle_not_after = None;
    a == b
}

pub async fn reconcile_audience(
    audience: Arc<SandboxAudience>,
    actx: Arc<AudienceCtx>,
) -> Result<Action, Error> {
    let ctx = &actx.ctx;
    // A replica that has lost the Lease writes nothing; it is about to exit (ADR-0011).
    if !ctx.leading() {
        return Ok(Action::await_change());
    }
    let ns = audience.namespace().ok_or(Error::NoNamespace)?;
    let name = audience.name_any();
    let now = Timestamp::now();

    // The version never goes backwards, even if status was lost: the published object is a
    // second source of truth, as for a policy.
    let cms: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &ns);
    let cm_name = audience_object_name(&name);
    let published = cms.get_opt(&cm_name).await?;
    let ann = |key: &str| {
        published
            .as_ref()
            .and_then(|c| c.metadata.annotations.as_ref())
            .and_then(|a| a.get(key).cloned())
    };
    let cm_version: Option<u64> = ann(ANN_VERSION).and_then(|v| v.parse().ok());
    let prev = audience.status.clone().unwrap_or_default();
    let (prev_version, prev_digest) = match (prev.bundle_version, cm_version) {
        (Some(s), Some(c)) if c > s => (Some(c), ann(ANN_DIGEST)),
        (Some(s), _) => (Some(s), prev.bundle_digest.clone()),
        (None, Some(c)) => (Some(c), ann(ANN_DIGEST)),
        (None, None) => (None, None),
    };

    let all = snapshot(&actx.audiences, Some(&audience));
    let catalog = AudienceCatalog::new(&actx.platform_namespace, &all);
    let out = evaluate_audience(&AudienceEvalInput {
        now,
        audience: &audience,
        catalog: &catalog,
        prev_version,
        prev_digest: prev_digest.as_deref(),
        prev_conditions: &prev.conditions,
        bundle_lifetime: ctx.bundle_lifetime,
    });
    for f in &out.findings {
        tracing::debug!(audience = %name, namespace = %ns, code = f.code, path = %f.path, "{}", f.message);
    }

    let mut status = out.status.clone();
    if out.bundle.is_some() {
        let ready = match &ctx.signer {
            Some(_) => condition(
                &prev.conditions,
                "Ready",
                true,
                "Published",
                "signed audience bundle published",
                now,
                audience.metadata.generation,
            ),
            None => condition(
                &prev.conditions,
                "Ready",
                false,
                "NoSigner",
                "no signing key configured: bundle not published",
                now,
                audience.metadata.generation,
            ),
        };
        if let Some(c) = status.conditions.iter_mut().find(|c| c.type_ == "Ready") {
            *c = ready;
        }
    }

    let stale = refresh::is_stale(prev.bundle_not_after, ctx.bundle_lifetime, now);
    let unchanged = same_modulo_expiry(&prev, &status)
        && !(stale && status.bundle_not_after != prev.bundle_not_after);
    if unchanged {
        let wait = refresh::next_check(
            out.requeue_after,
            prev.bundle_not_after,
            ctx.bundle_lifetime,
            now,
        );
        return Ok(Action::requeue(std_dur(wait)));
    }

    if let (Some(bundle), Some(signer)) = (&out.bundle, &ctx.signer) {
        // The guard's independent checks (ADR-0010, decision 4). An audience has no ceiling of its
        // own: the ceiling bounds each granting policy, checked when that policy publishes, so
        // `has_ceiling` is not a question here.
        let failed_checks = guard::check(
            &Evaluated {
                not_after: bundle.meta.not_after,
                has_ceiling: true,
                findings: &out.findings,
                active_grant_expiries: &[],
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
            tracing::error!(audience = %name, namespace = %ns, checks = ?failed_checks, "{msg}");
            refuse(ctx, &audience, &prev, &status, "FailOpenRefused", &msg, now).await?;
            return Ok(Action::requeue(REFUSED_REQUEUE));
        }
        if !ctx.leading() {
            return Ok(Action::await_change());
        }
        let envelope = match signer.sign_audience(bundle).await {
            Ok(e) => e,
            Err(e) => {
                // Fail closed: the previous bundle is still what consumers hold.
                ctx.counters.signing_failures.inc();
                refuse(
                    ctx,
                    &audience,
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
        let annotations = BTreeMap::from([
            (ANN_VERSION.to_string(), bundle.meta.version.to_string()),
            (ANN_DIGEST.to_string(), bundle.content_digest.clone()),
        ]);
        let labels = BTreeMap::from([(
            "app.kubernetes.io/managed-by".to_string(),
            FIELD_MANAGER.to_string(),
        )]);
        let cm = ConfigMap {
            metadata: ObjectMeta {
                name: Some(cm_name.clone()),
                namespace: Some(ns.clone()),
                labels: Some(labels),
                annotations: Some(annotations),
                // Owned by the audience: deleting it deletes the bundle (decision 10).
                owner_references: audience.controller_owner_ref(&()).map(|o| vec![o]),
                ..Default::default()
            },
            data: Some(BTreeMap::from([(
                AUDIENCE_BUNDLE_KEY.to_string(),
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

    Api::<SandboxAudience>::namespaced(ctx.client.clone(), &ns)
        .patch_status(
            &name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": status })),
        )
        .await?;
    record(
        ctx,
        &audience.object_ref(&()),
        events::audience(&prev, &status),
    )
    .await;
    Ok(Action::requeue(std_dur(out.requeue_after)))
}

/// Publish nothing: Ready false with `reason`, keeping the previous version and digest.
async fn refuse(
    ctx: &Ctx,
    audience: &SandboxAudience,
    prev: &SandboxAudienceStatus,
    status: &SandboxAudienceStatus,
    reason: &str,
    message: &str,
    now: Timestamp,
) -> Result<(), Error> {
    let ns = audience.namespace().ok_or(Error::NoNamespace)?;
    let failed = condition(
        &prev.conditions,
        "Ready",
        false,
        reason,
        message,
        now,
        audience.metadata.generation,
    );
    let mut conditions = status.conditions.clone();
    if let Some(c) = conditions.iter_mut().find(|c| c.type_ == "Ready") {
        *c = failed;
    }
    Api::<SandboxAudience>::namespaced(ctx.client.clone(), &ns)
        .patch_status(
            &audience.name_any(),
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": { "conditions": conditions } })),
        )
        .await?;
    let before = SandboxAudienceStatus {
        conditions: prev.conditions.clone(),
        ..Default::default()
    };
    let after = SandboxAudienceStatus {
        conditions,
        ..Default::default()
    };
    record(
        ctx,
        &audience.object_ref(&()),
        events::audience(&before, &after),
    )
    .await;
    Ok(())
}

pub fn error_audience(_obj: Arc<SandboxAudience>, err: &Error, actx: Arc<AudienceCtx>) -> Action {
    crate::reconcile::error_policy(_obj, err, actx.ctx.clone())
}
