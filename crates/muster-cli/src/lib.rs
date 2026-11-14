// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! The `muster` admin and diagnostics command-line tool.
//!
//! Data goes to stdout (text or JSON with `--format json`); messages and
//! hints go to stderr. Each command family is a module with a `run()`.

pub mod ctl;
pub mod practice;
pub mod token;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use muster_agent::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "muster",
    version,
    about = "Muster panic alarm: administration and diagnostics"
)]
pub struct Cli {
    /// Output format.
    #[arg(long, value_enum, global = true, default_value_t)]
    pub format: Format,
    /// Agent state directory (for offline admin commands).
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    /// Agent control socket (for `ctl` commands).
    #[arg(long, global = true)]
    pub control_socket: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Set up a practice.
    #[command(subcommand)]
    Practice(practice::PracticeCommand),
    /// Enrolment tokens.
    #[command(subcommand)]
    Token(token::TokenCommand),
    /// Talk to a running agent.
    #[command(subcommand)]
    Ctl(ctl::CtlCommand),
}

impl Cli {
    /// The data directory: `--data-dir`, then `MUSTER_DATA_DIR`, then the
    /// per-user default.
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .as_deref()
            .map(paths::expand)
            .or_else(|| std::env::var_os("MUSTER_DATA_DIR").map(PathBuf::from))
            .unwrap_or_else(|| paths::default_data_dir(false))
    }

    pub fn control_socket(&self) -> PathBuf {
        self.control_socket
            .as_deref()
            .map(paths::expand)
            .or_else(|| std::env::var_os("MUSTER_CONTROL_SOCKET").map(PathBuf::from))
            .unwrap_or_else(|| paths::default_control_socket(&self.data_dir()))
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match &cli.command {
        Command::Practice(c) => practice::run(&cli, c),
        Command::Token(c) => token::run(&cli, c),
        Command::Ctl(c) => ctl::run(&cli, c),
    }
}

/// Prints a JSON value per `--format`: pretty JSON, or a caller-supplied
/// text rendering.
pub fn emit(
    format: Format,
    v: &serde_json::Value,
    text: impl FnOnce(&serde_json::Value) -> String,
) {
    match format {
        Format::Json => println!("{}", serde_json::to_string_pretty(v).unwrap_or_default()),
        Format::Text => println!("{}", text(v)),
    }
}
