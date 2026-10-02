use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{Duration, Utc};
use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, ListParams, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Client, Resource, ResourceExt};
use sandboxpolicy_api::{
    GrantPhase, KnobGrant, KnobGrantStatus, SandboxPolicy, SandboxPolicyStatus, SandboxProfile,
    SandboxProfileStatus,
};
use sandboxpolicy_bundle::sign_bundle;
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
    let ns = policy.namespace().ok_or(Error::NoNamespace)?;
    let name = policy.name_any();
    let client = &ctx.client;
    let now = Utc::now();

    let profile = Api::<SandboxProfile>::all(client.clone())
        .get_opt(&policy.spec.profile_ref)
        .await?;
    let grants_api: Api<KnobGrant> = Api::namespaced(client.clone(), &ns);
    let grants = grants_api.list(&ListParams::default()).await?.items;

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
        let envelope = sign_bundle(bundle, &**signer)?;
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
    }

    Api::<SandboxPolicy>::namespaced(client.clone(), &ns)
        .patch_status(
            &name,
            &PatchParams::default(),
            &Patch::Merge(json!({ "status": status })),
        )
        .await?;
    Ok(Action::requeue(std_dur(out.requeue_after)))
}

pub async fn reconcile_profile(
    profile: Arc<SandboxProfile>,
    ctx: Arc<Ctx>,
) -> Result<Action, Error> {
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

pub fn error_policy<K>(_obj: Arc<K>, err: &Error, _ctx: Arc<Ctx>) -> Action {
    tracing::warn!(error = %err, "reconcile failed, retrying");
    Action::requeue(StdDuration::from_secs(30))
}
