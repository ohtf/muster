// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! `muster token create`: an enrolment token for managed deployment (flow B
//! in `spec/04-security.md`). Must run on an admin device's data directory.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Subcommand;
use muster_core::rng::CoreRng;
use muster_crypto::TrustStore;
use muster_crypto::token::{TokenRequest, create_token};
use muster_proto::{Cbor, Cert, CertSerial, TokenId};
use muster_store::Store;
use serde_json::json;

use crate::{Cli, emit};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

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
}

pub fn run(cli: &Cli, c: &TokenCommand) -> anyhow::Result<()> {
    let TokenCommand::Create {
        days,
        label,
        out,
        relays,
        seeds,
    } = c;
    if *days == 0 || *days > 365 {
        bail!("--days must be 1 to 365");
    }
    let dir = cli.data_dir();
    let store = Store::open(&dir.join("muster.db"))?;
    let keys = store
        .load_identity()?
        .context("this data directory has no identity")?;
    let state = store.load()?;
    let practice = state
        .practice
        .clone()
        .context("this device is not enrolled")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let mut trust = TrustStore::new(practice.practice_id, practice.root_pub);
    for raw in &state.certs {
        if let Ok(c) = Cert::from_cbor(raw) {
            let _ = trust.add_cert(c);
        }
    }
    if !trust.is_admin(&keys.device_id(), now) {
        bail!("this device is not a Practice Admin");
    }
    let key = state
        .group_keys
        .iter()
        .max_by_key(|k| (k.epoch, k.key.rotation_rank()))
        .context("no group key")?;
    let chain: Vec<Vec<u8>> = trust
        .chain_for(&keys.device_id(), now)
        .context("no valid admin chain")?
        .iter()
        .map(|c| c.to_cbor())
        .collect();
    let mut rng = CoreRng::from_seed(muster_agent::runtime::os_seed()?);
    let token_id = TokenId(rng.bytes());
    let (token, enroller_cert) = create_token(
        &keys,
        rng.bytes(),
        &key.key,
        TokenRequest {
            practice_id: practice.practice_id,
            practice_name: practice.name.clone(),
            root_pub: practice.root_pub,
            label: label.clone(),
            token_id,
            enroller_serial: CertSerial(rng.bytes()),
            issued_at: now,
            expires_at: now + days * DAY_MS,
            epoch: key.epoch,
            relay_urls: relays.clone(),
            seed_peers: seeds.clone(),
            network: Default::default(),
            issuer_chain: chain,
        },
    )?;
    store.apply(&muster_core::persist::PersistOp::AddCert(
        enroller_cert.to_cbor(),
    ))?;
    let s = token.to_token_string();
    let v = json!({
        "token_id": token_id.to_hex(),
        "expires_at": token.expires_at,
        "out": out.as_ref().map(|p| p.display().to_string()),
    });
    match out {
        Some(p) => {
            let p = muster_agent::paths::expand(p);
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
            eprintln!("Token {} (keep it secret).", token_id.to_hex());
        }
    }
    Ok(())
}
