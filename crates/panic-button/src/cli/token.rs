// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! `panic-button token create`: an enrolment token for managed deployment (flow B
//! in `spec/04-security.md`). Must run on an admin device's data directory.

use std::path::PathBuf;

use anyhow::Context;
use clap::Subcommand;
use serde_json::json;

use crate::cli::{Cli, emit};

#[derive(Debug, Subcommand)]
pub enum TokenCommand {
    /// Create a token. The token string is a secret: treat it like a password.
    Create {
        /// Days until it expires (default 30, at most 365).
        #[arg(long, default_value_t = 30)]
        days: u64,
        /// A label shown in the admin view, for example "IT rollout May".
        #[arg(long, default_value = "token")]
        label: String,
        /// Write the token to this file (mode 0600) instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Relay URLs to put in the token.
        #[arg(long = "relay")]
        relays: Vec<String>,
        /// Seed peers (host:port) to put in the token.
        #[arg(long = "seed")]
        seeds: Vec<String>,
    },
    /// List tokens and the devices each enrolled (asks the running agent).
    List,
    /// Revoke a token (admin; asks the running agent). Devices it already
    /// enrolled stay members: review them with `panic-button token list`.
    Revoke { token_id: String },
}

pub fn run(cli: &Cli, c: &TokenCommand) -> anyhow::Result<()> {
    match c {
        TokenCommand::List => {
            return crate::cli::ctl::run(cli, &crate::cli::ctl::CtlCommand::Tokens);
        }
        TokenCommand::Revoke { token_id } => {
            return crate::cli::ctl::run(
                cli,
                &crate::cli::ctl::CtlCommand::TokenRevoke {
                    token_id: token_id.clone(),
                },
            );
        }
        TokenCommand::Create { .. } => {}
    }
    let TokenCommand::Create {
        days,
        label,
        out,
        relays,
        seeds,
    } = c
    else {
        return Ok(());
    };
    let t = crate::agent::setup::create_enrolment_token(
        &cli.data_dir(),
        &crate::agent::setup::TokenOptions {
            days: *days,
            label: label.clone(),
            relay_urls: relays.clone(),
            seed_peers: seeds.clone(),
        },
    )?;
    let (s, token_id) = (t.text, t.token_id);
    let v = json!({
        "token_id": token_id,
        "expires_at": t.expires_at,
        "out": out.as_ref().map(|p| p.display().to_string()),
    });
    match out {
        Some(p) => {
            let p = crate::agent::paths::expand(p);
            let mut o = std::fs::OpenOptions::new();
            o.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                o.mode(0o600);
            }
            use std::io::Write;
            o.open(&p)
                .with_context(|| format!("creating {}", p.display()))?
                .write_all(s.as_bytes())?;
            emit(cli.format, &v, |v| {
                format!(
                    "Token {} written to {}. It stays valid until it expires or is revoked.",
                    v["token_id"].as_str().unwrap_or(""),
                    p.display()
                )
            });
        }
        None => {
            // The token itself is the data.
            println!("{s}");
            eprintln!("Token {token_id} (keep it secret).");
        }
    }
    Ok(())
}
