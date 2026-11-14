// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! `muster practice init`: set up a new practice on this device (flow A in
//! `spec/04-security.md`). The device becomes the first Practice Admin.

use std::path::PathBuf;

use anyhow::Context;
use clap::Subcommand;
use muster_crypto::recovery::check_passphrase;
use serde_json::json;

use crate::{Cli, emit};

#[derive(Debug, Subcommand)]
pub enum PracticeCommand {
    /// Create a practice; this device becomes its first admin.
    Init {
        /// Practice name, for example "Riverside Surgery".
        #[arg(long)]
        name: String,
        /// Where to write the recovery kit (keep it offline and safe).
        #[arg(long)]
        recovery_kit: PathBuf,
        /// File holding the recovery passphrase (at least 12 characters).
        /// Read from MUSTER_PASSPHRASE if not given.
        #[arg(long)]
        passphrase_file: Option<PathBuf>,
        /// This device's label [default: host name].
        #[arg(long)]
        label: Option<String>,
    },
}

fn passphrase(file: &Option<PathBuf>) -> anyhow::Result<String> {
    let p = match file {
        Some(f) => std::fs::read_to_string(muster_agent::paths::expand(f))
            .with_context(|| format!("reading {}", f.display()))?,
        None => std::env::var("MUSTER_PASSPHRASE")
            .context("give --passphrase-file or set MUSTER_PASSPHRASE")?,
    };
    let p = p.trim_end_matches(['\n', '\r']).to_owned();
    check_passphrase(&p)?;
    Ok(p)
}

pub fn run(cli: &Cli, c: &PracticeCommand) -> anyhow::Result<()> {
    match c {
        PracticeCommand::Init {
            name,
            recovery_kit,
            passphrase_file,
            label,
        } => {
            let pass = passphrase(passphrase_file)?;
            let label = label.clone().unwrap_or_else(|| "admin".into());
            eprintln!("Deriving the recovery kit key (this takes a few seconds)...");
            let kit = muster_agent::paths::expand(recovery_kit);
            let p = muster_agent::setup::init_practice(&cli.data_dir(), name, &label, &pass, &kit)?;
            let v = json!({
                "practice": p.name,
                "practice_id": p.practice_id,
                "root_pub": p.root_pub,
                "admin_device": p.admin_device,
                "recovery_kit": p.recovery_kit.display().to_string(),
            });
            emit(cli.format, &v, |v| {
                format!(
                    "Created {} ({}).\nThis device is its first admin.\nRecovery kit: {}\nStore it offline. Without it and its passphrase, lost admin access cannot be recovered.",
                    v["practice"].as_str().unwrap_or(""),
                    v["practice_id"].as_str().unwrap_or(""),
                    v["recovery_kit"].as_str().unwrap_or("")
                )
            });
            Ok(())
        }
    }
}
