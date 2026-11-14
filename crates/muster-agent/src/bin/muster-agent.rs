// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! `muster-agent`: the headless agent (FR-PLT-005) for Linux relay boxes,
//! integration nodes and tests.

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use muster_agent::config::ConfigFile;
use muster_agent::paths;

#[derive(Debug, Parser)]
#[command(version, about = "Muster headless agent")]
struct Args {
    /// Run without a desktop session (required in v1).
    #[arg(long)]
    headless: bool,
    /// Machine config file [default: platform location; optional].
    #[arg(long)]
    config: Option<PathBuf>,
    /// Machine policy file, applied over the config [default: /etc/muster/policy.toml if present].
    #[arg(long)]
    policy: Option<PathBuf>,
    /// State directory [default: /var/lib/muster for --headless].
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Control socket path [default: $XDG_RUNTIME_DIR/muster/control.sock].
    #[arg(long)]
    control_socket: Option<PathBuf>,
    /// Device label [default: the label it was enrolled with, else the
    /// host name].
    #[arg(long)]
    label: Option<String>,
    /// Allow `raise` over the control API (tests and integration nodes).
    #[arg(long)]
    allow_raise: bool,
}

fn load(path: Option<PathBuf>, default: PathBuf) -> anyhow::Result<ConfigFile> {
    match path {
        Some(p) => Ok(ConfigFile::load(&paths::expand(&p))?),
        None if default.exists() => Ok(ConfigFile::load(&default)?),
        None => Ok(ConfigFile::default()),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("MUSTER_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();
    let args = Args::parse();
    if !args.headless {
        anyhow::bail!("the desktop agent is the Muster app; run this binary with --headless");
    }
    let config = load(args.config, paths::default_config_file())?;
    let policy = load(args.policy, paths::default_policy_file())?;
    let config = config.with_policy(&policy);
    let data_dir = args
        .data_dir
        .map(|p| paths::expand(&p))
        .unwrap_or_else(|| paths::default_data_dir(true));
    let control_socket = args
        .control_socket
        .map(|p| paths::expand(&p))
        .unwrap_or_else(|| paths::default_control_socket(&data_dir));
    muster_agent::run(muster_agent::AgentOptions {
        data_dir,
        control_socket,
        config,
        allow_raise: args.allow_raise,
        label: args.label,
    })
    .await
    .context("agent stopped")
}
