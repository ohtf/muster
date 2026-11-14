// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! `muster practice init`: set up a new practice on this device (flow A in
//! `spec/04-security.md`). The device becomes the first Practice Admin.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Subcommand;
use muster_core::bootstrap::{new_device_keys, new_practice};
use muster_core::rng::CoreRng;
use muster_crypto::recovery::{KdfParams, RecoveryContents, check_passphrase, open_kit, seal_kit};
use muster_proto::Cbor;
use muster_store::Store;
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

fn wall_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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

fn write_private(path: &std::path::Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .with_context(|| format!("creating {} (it must not already exist)", path.display()))?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
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
            let dir = cli.data_dir();
            std::fs::create_dir_all(&dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
            }
            let mut store = Store::open(&dir.join("muster.db"))?;
            if store.load()?.practice.is_some() {
                bail!("{} already belongs to a practice", dir.display());
            }
            let mut rng = CoreRng::from_seed(muster_agent::runtime::os_seed()?);
            let keys = match store.load_identity()? {
                Some(k) => k,
                None => {
                    let k = new_device_keys(&mut rng);
                    store.save_identity(&k)?;
                    k
                }
            };
            let label = label.clone().unwrap_or_else(|| "admin".into());
            let now = wall_now();
            let np = new_practice(&mut rng, &keys, name, &label, now);
            let practice = np.state.practice.clone().context("practice")?;
            eprintln!("Deriving the recovery kit key (this takes a few seconds)...");
            let contents = RecoveryContents {
                practice_id: practice.practice_id,
                practice_name: practice.name.clone(),
                root: np.root,
                created_at: now,
            };
            let kit = seal_kit(&contents, &pass, rng.bytes(), rng.bytes(), KdfParams::V1)?;
            // Prove the passphrase opens it before relying on it.
            let check = open_kit(&kit, &pass)?;
            if check.root.public() != practice.root_pub {
                bail!("recovery kit check failed");
            }
            drop(contents);
            let kit_path = muster_agent::paths::expand(recovery_kit);
            write_private(&kit_path, &kit.to_cbor())?;
            store.apply_all(np.state.to_ops().iter())?;
            let v = json!({
                "practice": practice.name,
                "practice_id": practice.practice_id.to_hex(),
                "root_pub": muster_proto::DeviceId(practice.root_pub).to_hex(),
                "admin_device": keys.device_id().to_hex(),
                "recovery_kit": kit_path.display().to_string(),
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
