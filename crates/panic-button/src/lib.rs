// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The `panic-button` admin and diagnostics command-line tool.
//!
//! Data goes to stdout (text or JSON with `--format json`); messages and
//! hints go to stderr. Each command family is a module with a `run()`.

pub mod ctl;
pub mod practice;
pub mod token;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use panic_button_agent::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "panic-button",
    version,
    about = "Panic Button panic alarm: administration and diagnostics"
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
    /// Devices in the practice (asks the running agent).
    #[command(subcommand)]
    Device(DeviceCommand),
}

#[derive(Debug, Subcommand)]
pub enum DeviceCommand {
    /// Every device in the practice, heard or not.
    List,
    /// Make a device a Practice Admin (admin).
    Promote { device: String },
    /// Rename a device (admin).
    Rename { device: String, label: String },
    /// Set a device's room (admin).
    Room { device: String, room: String },
    /// Remove a device (admin). `--compromised` also removes everything it enrolled.
    Revoke {
        device: String,
        #[arg(long)]
        compromised: bool,
        #[arg(long, default_value = "")]
        reason: String,
    },
}

impl Cli {
    /// The data directory: `--data-dir`, then `PANIC_BUTTON_DATA_DIR`, then the
    /// per-user default.
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .as_deref()
            .map(paths::expand)
            .or_else(|| std::env::var_os("PANIC_BUTTON_DATA_DIR").map(PathBuf::from))
            .unwrap_or_else(|| paths::default_data_dir(false))
    }

    pub fn control_socket(&self) -> PathBuf {
        self.control_socket
            .as_deref()
            .map(paths::expand)
            .or_else(|| std::env::var_os("PANIC_BUTTON_CONTROL_SOCKET").map(PathBuf::from))
            .unwrap_or_else(|| paths::default_control_socket(&self.data_dir()))
    }
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match &cli.command {
        Command::Practice(c) => practice::run(&cli, c),
        Command::Token(c) => token::run(&cli, c),
        Command::Ctl(c) => ctl::run(&cli, c),
        Command::Device(DeviceCommand::List) => ctl::run(&cli, &ctl::CtlCommand::Devices),
        Command::Device(DeviceCommand::Rename { device, label }) => ctl::run(
            &cli,
            &ctl::CtlCommand::DeviceSettings {
                device: device.clone(),
                label: Some(label.clone()),
                room: None,
            },
        ),
        Command::Device(DeviceCommand::Room { device, room }) => ctl::run(
            &cli,
            &ctl::CtlCommand::DeviceSettings {
                device: device.clone(),
                label: None,
                room: Some(room.clone()),
            },
        ),
        Command::Device(DeviceCommand::Promote { device }) => ctl::run(
            &cli,
            &ctl::CtlCommand::Promote {
                device: device.clone(),
            },
        ),
        Command::Device(DeviceCommand::Revoke {
            device,
            compromised,
            reason,
        }) => ctl::run(
            &cli,
            &ctl::CtlCommand::Revoke {
                device: device.clone(),
                compromised: *compromised,
                reason: reason.clone(),
            },
        ),
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
