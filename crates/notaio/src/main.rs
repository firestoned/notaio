// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Notaio: validates SandboxProfile, SandboxPolicy and KnobGrant objects, compiles each
//! policy into a signed, versioned bundle and publishes it. It holds no hypervisor credentials and
//! no agent traffic ever reaches it. See docs/architecture.md.
//!
//! One binary, the roles as subcommands (ADR-0006): `notaio controller` runs the controller,
//! `notaio lint` checks policy files for pull requests.

mod error;
mod reconcile;
mod signer;

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Duration;
use clap::Parser;
use futures::StreamExt;
use kube::runtime::controller::Controller;
use kube::runtime::events::{Recorder, Reporter};
use kube::runtime::reflector::{self, ObjectRef, Store};
use kube::runtime::{watcher, WatchStreamExt};
use kube::{Api, Client};
use notaio::cli::{
    Cli, Command, Format, LintArgs, DEFAULT_BUNDLE_LIFETIME_SECONDS, DEFAULT_NAMESPACE,
    ENV_BUNDLE_LIFETIME, ENV_HTTP_ADDR, ENV_LOG_JSON, ENV_NAMESPACE, ENV_POD_NAME,
};
use notaio::http::Observed;
use notaio::leader::Elector;
use notaio::metrics::{Counters, Snapshot};
use sandboxpolicy_api::{ExposureCeiling, KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::ceilings::Ceilings;
use tracing_subscriber::EnvFilter;

use reconcile::{error_policy, reconcile_policy, reconcile_profile, Ctx};

/// `notaio lint` exit codes (ADR-0006, decision 6). 0 is success.
const EXIT_FINDINGS: u8 = 1;
const EXIT_INPUT: u8 = 2;

fn main() -> ExitCode {
    // A bare `notaio` or a bad flag prints usage and exits 2, from clap.
    let cli = Cli::parse();
    match cli.command {
        Command::Controller => match run_controller() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("notaio controller: {e:#}");
                ExitCode::FAILURE
            }
        },
        Command::Lint(args) => run_lint(&args),
    }
}

/// What the health and metrics listener reads: the controller's own caches (ADR-0010).
struct Observer {
    leading: Arc<AtomicBool>,
    policies: Store<SandboxPolicy>,
    profiles: Store<SandboxProfile>,
    grants: Store<KnobGrant>,
    counters: Counters,
    bundle_lifetime: Duration,
    ready: AtomicBool,
}

impl Observed for Observer {
    fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    fn metrics(&self) -> String {
        self.counters
            .leader
            .set(i64::from(self.leading.load(Ordering::Acquire)));
        notaio::metrics::render(
            &self.counters,
            &Snapshot {
                policies: &self.policies.state(),
                profiles: &self.profiles.state(),
                grants: &self.grants.state(),
                now: chrono::Utc::now(),
                bundle_lifetime: self.bundle_lifetime,
            },
        )
    }
}

/// A cache of every `K`, and the watch that fills it. Watch errors are retried with backoff.
fn cache<K>(api: Api<K>, kind: &'static str) -> (Store<K>, impl std::future::Future<Output = ()>)
where
    K: kube::Resource
        + Clone
        + serde::de::DeserializeOwned
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
    K::DynamicType: Default + Eq + std::hash::Hash + Clone,
{
    let (store, writer) = reflector::store();
    let task = reflector::reflector(writer, watcher(api, watcher::Config::default()))
        .default_backoff()
        .touched_objects()
        .for_each(move |res| async move {
            if let Err(e) = res {
                tracing::warn!(error = %e, kind, "cache watch error");
            }
        });
    (store, task)
}

fn run_lint(args: &LintArgs) -> ExitCode {
    let now = args.now.unwrap_or_else(chrono::Utc::now);
    let report = match notaio::lint::load(&args.paths)
        .and_then(|sources| notaio::lint::lint_sources(&sources, now))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("notaio lint: {}: {}", e.path.display(), e.message);
            return ExitCode::from(EXIT_INPUT);
        }
    };
    let out = match args.format {
        Format::Text => notaio::lint::render_text(&report),
        Format::Sarif => notaio::lint::render_sarif(&report),
    };
    print!("{out}");
    if report.has_errors() {
        return ExitCode::from(EXIT_FINDINGS);
    }
    ExitCode::SUCCESS
}

#[tokio::main]
async fn run_controller() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if std::env::var(ENV_LOG_JSON).is_ok_and(|v| v == "1") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let lifetime_secs: i64 = std::env::var(ENV_BUNDLE_LIFETIME)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_BUNDLE_LIFETIME_SECONDS);
    let signer = signer::from_env()?;
    match &signer {
        Some(s) => tracing::info!(signer = %s.describe(), "signing bundles"),
        None => tracing::warn!(
            "no signing key configured: policies are validated but no bundle is published"
        ),
    }

    // Two replicas with one identity would both lead, so there is no default (ADR-0011).
    let identity = std::env::var(ENV_POD_NAME).map_err(|_| {
        anyhow::anyhow!(
            "{ENV_POD_NAME} is required: it is this replica's identity in leader election"
        )
    })?;
    let namespace = std::env::var(ENV_NAMESPACE).unwrap_or_else(|_| DEFAULT_NAMESPACE.to_string());

    let client = Client::try_default().await?;
    let elector = Elector::new(client.clone(), &namespace, &identity);
    let counters = Counters::default();
    let reporter = Reporter {
        controller: "notaio".to_string(),
        instance: Some(identity.clone()),
    };
    let ctx = Arc::new(Ctx {
        leading: elector.leading(),
        client: client.clone(),
        signer,
        ceilings: Ceilings::default(),
        bundle_lifetime: Duration::seconds(lifetime_secs),
        counters: counters.clone(),
        recorder: Recorder::new(client.clone(), reporter),
    });

    // Bind before anything else, so a taken port fails the start rather than the first probe.
    let addr =
        std::env::var(ENV_HTTP_ADDR).unwrap_or_else(|_| notaio::http::DEFAULT_ADDR.to_string());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("binding the health and metrics listener on {addr}: {e}"))?;

    let policies: Api<SandboxPolicy> = Api::all(client.clone());
    let profiles: Api<SandboxProfile> = Api::all(client.clone());
    let grants: Api<KnobGrant> = Api::all(client.clone());
    // The listener's own caches, one per kind, for readiness and the gauges. Not the
    // controllers' stores: a store's readiness wakes only one waiter, and the controller is
    // already waiting on its own, so a second waiter there could stall either of them.
    let (policy_cache, policy_cache_task) = cache(policies.clone(), "policy");
    let (profile_cache, profile_cache_task) = cache(profiles.clone(), "profile");
    let (grant_cache, grant_cache_task) = cache(grants.clone(), "grant");
    let exposure_ceilings: Api<ExposureCeiling> = Api::all(client.clone());

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
        // A ceiling change re-evaluates the policy with its name (ADR-0007).
        .watches(exposure_ceilings, watcher::Config::default(), |c| {
            let mut r = ObjectRef::<SandboxPolicy>::new(&kube::ResourceExt::name_any(&c));
            r.namespace = kube::ResourceExt::namespace(&c);
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

    let profile_controller = Controller::new(profiles, watcher::Config::default());
    let observer = Arc::new(Observer {
        leading: elector.leading(),
        policies: policy_cache,
        profiles: profile_cache,
        grants: grant_cache,
        counters,
        bundle_lifetime: ctx.bundle_lifetime,
        ready: AtomicBool::new(false),
    });
    tokio::spawn({
        let o = observer.clone();
        async move {
            let synced = o.policies.wait_until_ready().await.is_ok()
                && o.profiles.wait_until_ready().await.is_ok()
                && o.grants.wait_until_ready().await.is_ok();
            if synced {
                o.ready.store(true, Ordering::Release);
                tracing::info!("every watch has its first list: ready");
            }
        }
    });
    tracing::info!(%addr, "serving /healthz, /readyz and /metrics");
    let server = tokio::spawn(notaio::http::serve(listener, observer));

    let profile_task = profile_controller
        .shutdown_on_signal()
        .run(reconcile_profile, error_policy, ctx)
        .for_each(|res| async move {
            if let Err(e) = res {
                tracing::warn!(error = %e, "profile controller error");
            }
        });

    // Every replica keeps its caches and serves health and metrics.
    let caches = tokio::spawn(async move {
        futures::join!(policy_cache_task, profile_cache_task, grant_cache_task);
    });

    // Only the Lease holder reconciles, signs and publishes (ADR-0011). The controllers are lazy
    // streams: nothing watches or reconciles until they are polled below.
    tracing::info!(%identity, %namespace, "notaio started: contending for the Lease");
    tokio::select! {
        () = elector.acquire() => {}
        () = shutdown_signal() => {
            server.abort();
            caches.abort();
            return Ok(());
        }
    }
    let outcome = tokio::select! {
        () = async { futures::join!(policy_task, profile_task); } => Ok(()),
        // Losing the Lease ends the process; the pod restarts as a follower.
        reason = elector.hold() => Err(anyhow::anyhow!("stopped leading: {reason}")),
    };
    if outcome.is_ok() {
        elector.release().await;
    }
    server.abort();
    caches.abort();
    outcome
}

/// SIGTERM or Ctrl-C, for a replica that is not leading. The controllers handle their own.
async fn shutdown_signal() {
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => futures::future::pending::<()>().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = term => {}
    }
}
