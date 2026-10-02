//! Notaio: validates SandboxProfile, SandboxPolicy and KnobGrant objects, compiles each
//! policy into a signed, versioned bundle and publishes it. It holds no hypervisor credentials and
//! no agent traffic ever reaches it. See docs/architecture.md.

mod error;
mod reconcile;
mod signer;

use std::sync::Arc;

use chrono::Duration;
use futures::StreamExt;
use kube::runtime::controller::Controller;
use kube::runtime::reflector::ObjectRef;
use kube::runtime::watcher;
use kube::{Api, Client};
use sandboxpolicy_api::{KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::ceilings::Ceilings;
use tracing_subscriber::EnvFilter;

use reconcile::{error_policy, reconcile_policy, reconcile_profile, Ctx};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if std::env::var("NOTAIO_LOG_JSON").is_ok_and(|v| v == "1") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let lifetime_secs: i64 = std::env::var("NOTAIO_BUNDLE_LIFETIME_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3600);
    let signer = signer::from_env()?;
    if signer.is_none() {
        tracing::warn!(
            "no signing key configured: policies are validated but no bundle is published"
        );
    }

    let client = Client::try_default().await?;
    let ctx = Arc::new(Ctx {
        client: client.clone(),
        signer,
        ceilings: Ceilings::default(),
        bundle_lifetime: Duration::seconds(lifetime_secs),
    });

    let policies: Api<SandboxPolicy> = Api::all(client.clone());
    let profiles: Api<SandboxProfile> = Api::all(client.clone());
    let grants: Api<KnobGrant> = Api::all(client.clone());

    let policy_controller = Controller::new(policies, watcher::Config::default());
    let store = policy_controller.store();
    let policy_task = policy_controller
        // A profile change re-evaluates every policy that references it.
        .watches(profiles.clone(), watcher::Config::default(), move |p| {
            let name = kube::ResourceExt::name_any(&p);
            store
                .state()
                .into_iter()
                .filter(|pol| pol.spec.profile_ref == name)
                .map(|pol| ObjectRef::from_obj(&*pol))
                .collect::<Vec<_>>()
        })
        // A grant change re-evaluates the policy it names.
        .watches(grants, watcher::Config::default(), |g| {
            let ns = kube::ResourceExt::namespace(&g);
            let mut r = ObjectRef::<SandboxPolicy>::new(&g.spec.policy_ref);
            r.namespace = ns;
            Some(r)
        })
        .shutdown_on_signal()
        .run(reconcile_policy, error_policy, ctx.clone())
        .for_each(|res| async move {
            match res {
                Ok((obj, _)) => tracing::debug!(policy = %obj.name, "reconciled"),
                Err(e) => tracing::warn!(error = %e, "policy controller error"),
            }
        });

    let profile_task = Controller::new(profiles, watcher::Config::default())
        .shutdown_on_signal()
        .run(reconcile_profile, error_policy, ctx)
        .for_each(|res| async move {
            if let Err(e) = res {
                tracing::warn!(error = %e, "profile controller error");
            }
        });

    tracing::info!("notaio started");
    futures::join!(policy_task, profile_task);
    Ok(())
}
