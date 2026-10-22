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
use std::sync::Arc;

use chrono::Duration;
use clap::Parser;
use futures::StreamExt;
use kube::runtime::controller::Controller;
use kube::runtime::reflector::ObjectRef;
use kube::runtime::watcher;
use kube::{Api, Client};
use notaio::cli::{Cli, Command, Format, LintArgs};
use sandboxpolicy_api::{ExposureCeiling, KnobGrant, SandboxPolicy, SandboxProfile};
use sandboxpolicy_core::ceilings::Ceilings;
use tracing_subscriber::EnvFilter;

use reconcile::{error_policy, reconcile_policy, reconcile_profile, Ctx};

/// Bundle lifetime when `NOTAIO_BUNDLE_LIFETIME_SECONDS` is not set. A consumer refuses a bundle
/// past `notAfter`, so this bounds how long a tightening takes to land (ADR-0002, decision 3).
const DEFAULT_BUNDLE_LIFETIME_SECONDS: i64 = 3600;

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
        .unwrap_or(DEFAULT_BUNDLE_LIFETIME_SECONDS);
    let signer = signer::from_env()?;
    match &signer {
        Some(s) => tracing::info!(signer = %s.describe(), "signing bundles"),
        None => tracing::warn!(
            "no signing key configured: policies are validated but no bundle is published"
        ),
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
