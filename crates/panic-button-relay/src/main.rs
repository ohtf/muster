// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! `panic-button-relay`: serve, and manage tenants.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use panic_button_relay::config::{RelayConfig, TenantConfig, TenantsFile};
use panic_button_relay::persist::Store;
use panic_button_relay::server::{App, now_ms, router, spawn_housekeeping};
use panic_button_relay::state::Relay;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Panic Button relay: a blind forwarder of sealed envelopes"
)]
struct Args {
    /// Config file.
    #[arg(long, default_value = "/etc/panic-button-relay/relay.toml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run the relay.
    Serve,
    /// Manage tenants (practices this relay serves).
    #[command(subcommand)]
    Tenant(TenantCmd),
}

#[derive(Debug, Subcommand)]
enum TenantCmd {
    Add {
        #[arg(long)]
        practice_id: String,
        #[arg(long)]
        root_pub: String,
        #[arg(long, default_value = "")]
        name: String,
    },
    List,
    Remove {
        #[arg(long)]
        practice_id: String,
    },
}

fn load(path: &std::path::Path) -> anyhow::Result<RelayConfig> {
    if path.exists() {
        RelayConfig::load(path).with_context(|| format!("loading {}", path.display()))
    } else {
        Ok(RelayConfig::default())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("PANIC_BUTTON_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();
    let args = Args::parse();
    let cfg = load(&args.config)?;
    match args.command {
        Cmd::Serve => {
            let mut relay = Relay::new(cfg.limits.clone());
            let file = TenantsFile::load(&cfg.tenants_file())?;
            let now = now_ms();
            for t in cfg.tenants.iter().chain(file.tenants.iter()) {
                let (pid, root) = t.parsed().map_err(anyhow::Error::msg)?;
                relay.add_tenant(pid, root, &t.name, now);
            }
            if cfg.persist {
                let path = cfg.data_dir.join("relay.sqlite");
                let store =
                    Store::open(&path).with_context(|| format!("opening {}", path.display()))?;
                relay.attach_store(store, now)?;
                tracing::info!("persisting to {}", path.display());
            }
            tracing::info!("serving {} tenants on {}", relay.tenant_count(), cfg.listen);
            let app = App::new(relay, cfg.source_url());
            spawn_housekeeping(app.clone());
            let listener = tokio::net::TcpListener::bind(cfg.listen).await?;
            axum::serve(listener, router(app))
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
            Ok(())
        }
        Cmd::Tenant(t) => {
            let path = cfg.tenants_file();
            let mut file = TenantsFile::load(&path)?;
            match t {
                TenantCmd::Add {
                    practice_id,
                    root_pub,
                    name,
                } => {
                    let t = TenantConfig {
                        practice_id: practice_id.to_lowercase(),
                        root_pub: root_pub.to_lowercase(),
                        name,
                    };
                    t.parsed().map_err(anyhow::Error::msg)?;
                    if file.tenants.iter().any(|x| x.practice_id == t.practice_id) {
                        bail!("tenant {} already exists", t.practice_id);
                    }
                    file.tenants.push(t);
                    file.save(&path)?;
                    eprintln!("Added. Restart the relay to apply.");
                }
                TenantCmd::List => {
                    for t in cfg.tenants.iter().chain(file.tenants.iter()) {
                        println!("{}  {}", t.practice_id, t.name);
                    }
                }
                TenantCmd::Remove { practice_id } => {
                    let before = file.tenants.len();
                    file.tenants
                        .retain(|t| t.practice_id != practice_id.to_lowercase());
                    if file.tenants.len() == before {
                        bail!("no tenant {practice_id} in {}", path.display());
                    }
                    file.save(&path)?;
                    eprintln!("Removed. Restart the relay to apply.");
                }
            }
            Ok(())
        }
    }
}
