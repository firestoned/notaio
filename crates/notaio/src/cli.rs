//! The `notaio` command line (ADR-0006): one binary, the roles as subcommands.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};

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
    /// Check SandboxProfile, SandboxPolicy and KnobGrant files, for pull requests.
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
