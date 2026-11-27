// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! The `notaio` command line (ADR-0006): one binary, the roles as subcommands.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use jiff::Timestamp;
use sandboxpolicy_bundle::SchemaVersion;

/// How long a bundle stays valid, in seconds. A consumer refuses a bundle past `notAfter`, so this
/// bounds how long a policy that turns invalid stays honoured, and how long the signer may be down
/// before bundles expire (ADR-0018).
pub const ENV_BUNDLE_LIFETIME: &str = "NOTAIO_BUNDLE_LIFETIME_SECONDS";
/// 15 minutes: the shortest lifetime whose signer-outage budget a person can act within
/// (ADR-0018, decision 1).
pub const DEFAULT_BUNDLE_LIFETIME_SECONDS: i64 = 900;
/// Below 5 minutes a brief signer restart already expires bundles (ADR-0018, decision 2).
pub const MIN_BUNDLE_LIFETIME_SECONDS: i64 = 300;
/// Above a day an invalid policy stays honoured long after anyone watches for it (ADR-0018). The
/// same constant consumers refuse a longer lifetime by (ADR-0002, decision 5), so the two cannot
/// drift apart.
pub const MAX_BUNDLE_LIFETIME_SECONDS: i64 = sandboxpolicy_bundle::MAX_BUNDLE_LIFETIME_SECONDS;

/// The bundle lifetime from the raw value of [`ENV_BUNDLE_LIFETIME`], or the default when unset.
///
/// # Errors
/// A value that is not a whole number of seconds, or is outside
/// [`MIN_BUNDLE_LIFETIME_SECONDS`]..=[`MAX_BUNDLE_LIFETIME_SECONDS`]: the controller refuses to
/// start rather than fall back to a lifetime nobody chose (ADR-0018, decision 2).
pub fn bundle_lifetime_seconds(raw: Option<&str>) -> anyhow::Result<i64> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_BUNDLE_LIFETIME_SECONDS);
    };
    let secs: i64 = raw.trim().parse().map_err(|_| {
        anyhow::anyhow!("{ENV_BUNDLE_LIFETIME}={raw:?} is not a whole number of seconds")
    })?;
    if !(MIN_BUNDLE_LIFETIME_SECONDS..=MAX_BUNDLE_LIFETIME_SECONDS).contains(&secs) {
        anyhow::bail!(
            "{ENV_BUNDLE_LIFETIME}={secs} is outside {MIN_BUNDLE_LIFETIME_SECONDS} to \
             {MAX_BUNDLE_LIFETIME_SECONDS} seconds"
        );
    }
    Ok(secs)
}
/// The bundle schema the controller emits, `v3` or `v4` (ADR-0026 decision 5). An operator turns
/// `v4` on once every consumer reads it, and back off to roll back: the version keeps climbing
/// either way.
pub const ENV_BUNDLE_SCHEMA: &str = "NOTAIO_BUNDLE_SCHEMA";
/// What an unset [`ENV_BUNDLE_SCHEMA`] means in this release: v3, so no consumer sees v4 until
/// someone chooses it (ADR-0026 decision 5).
pub const DEFAULT_BUNDLE_SCHEMA: SchemaVersion = SchemaVersion::V3;

/// The bundle schema from the raw value of [`ENV_BUNDLE_SCHEMA`], or [`DEFAULT_BUNDLE_SCHEMA`]
/// when unset.
///
/// # Errors
/// Anything but `v3` or `v4`, an empty value included: the controller refuses to start rather than
/// emit a schema nobody chose, which every consumer would refuse.
pub fn bundle_schema(raw: Option<&str>) -> anyhow::Result<SchemaVersion> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_BUNDLE_SCHEMA);
    };
    match raw.trim() {
        "v3" => Ok(SchemaVersion::V3),
        "v4" => Ok(SchemaVersion::V4),
        _ => anyhow::bail!("{ENV_BUNDLE_SCHEMA}={raw:?} is neither v3 nor v4"),
    }
}

/// The platform audience namespace (ADR-0020, decision 2). Unset, `SandboxAudience` is not
/// reconciled and policies behave as before the ADR; set, every audience rule applies. No default:
/// the consumer names the same namespace, and a guess on either side is a refusal.
pub const ENV_AUDIENCE_PLATFORM_NAMESPACE: &str = "NOTAIO_AUDIENCE_PLATFORM_NAMESPACE";
/// The longest Kubernetes namespace name (an RFC 1123 label).
const MAX_NAMESPACE_LEN: usize = 63;

/// The platform audience namespace from the raw value of [`ENV_AUDIENCE_PLATFORM_NAMESPACE`]:
/// `None` when unset or empty, which leaves audiences off.
///
/// # Errors
/// A value that is not a Kubernetes namespace name: the controller refuses to start rather than
/// classify audiences against a namespace that cannot exist.
pub fn audience_platform_namespace(raw: Option<&str>) -> anyhow::Result<Option<String>> {
    let Some(ns) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let label = ns.len() <= MAX_NAMESPACE_LEN
        && ns
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !ns.starts_with('-')
        && !ns.ends_with('-');
    if !label {
        anyhow::bail!("{ENV_AUDIENCE_PLATFORM_NAMESPACE}={ns:?} is not a namespace name");
    }
    Ok(Some(ns.to_string()))
}

/// `1` for JSON logs.
pub const ENV_LOG_JSON: &str = "NOTAIO_LOG_JSON";
/// The health and metrics listener's address (ADR-0010). Default `crate::http::DEFAULT_ADDR`.
pub const ENV_HTTP_ADDR: &str = "NOTAIO_HTTP_ADDR";
/// The pod's name, from the downward API: this replica's identity in leader election (ADR-0011)
/// and in the Events it records. Required by `notaio controller`.
pub const ENV_POD_NAME: &str = "NOTAIO_POD_NAME";
/// The namespace that holds the leader Lease: the pod's own, from the downward API.
pub const ENV_NAMESPACE: &str = "NOTAIO_NAMESPACE";
pub const DEFAULT_NAMESPACE: &str = "notaio-system";

#[derive(Debug, Parser)]
#[command(
    name = "notaio",
    version,
    about = "Sandbox policy compiler and validator"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the controller: watch policy objects, publish signed bundles.
    Controller,
    /// Check SandboxProfile, SandboxPolicy, KnobGrant, ExposureCeiling and SandboxAudience files,
    /// for pull requests.
    Lint(LintArgs),
}

#[derive(Debug, clap::Args)]
pub struct LintArgs {
    /// Files or directories to check. Directories are searched for *.yaml and *.yml.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Evaluate grants at this instant (RFC 3339) instead of now.
    #[arg(long)]
    pub now: Option<Timestamp>,
    /// The platform audience namespace (ADR-0020), as the controller's
    /// NOTAIO_AUDIENCE_PLATFORM_NAMESPACE. With it, SandboxAudience objects are linted and policy
    /// grants are checked against their definitions; without it, neither is.
    #[arg(long)]
    pub platform_audience_namespace: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Sarif,
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod cli_tests;
