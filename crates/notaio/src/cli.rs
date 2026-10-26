//! The `notaio` command line (ADR-0006): one binary, the roles as subcommands.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};

/// How long a bundle stays valid, in seconds. A consumer refuses a bundle past `notAfter`, so this
/// bounds how long a tightening takes to land (ADR-0002, decision 3).
pub const ENV_BUNDLE_LIFETIME: &str = "NOTAIO_BUNDLE_LIFETIME_SECONDS";
pub const DEFAULT_BUNDLE_LIFETIME_SECONDS: i64 = 3600;
/// `1` for JSON logs.
pub const ENV_LOG_JSON: &str = "NOTAIO_LOG_JSON";
/// The health and metrics listener's address (ADR-0010). Default `crate::http::DEFAULT_ADDR`.
pub const ENV_HTTP_ADDR: &str = "NOTAIO_HTTP_ADDR";
/// The pod's name, from the downward API, naming the replica in Events.
pub const ENV_POD_NAME: &str = "NOTAIO_POD_NAME";

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
    /// Check SandboxProfile, SandboxPolicy, KnobGrant and ExposureCeiling files, for pull requests.
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
    pub now: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Text,
    Sarif,
}
