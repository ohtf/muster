// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! `panic-button practice init`: set up a new practice on this device (flow A in
//! `spec/04-security.md`). The device becomes the first Practice Admin.

use std::path::PathBuf;

use anyhow::Context;
use clap::Subcommand;
use panic_button_crypto::recovery::check_passphrase;
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
        /// Read from PANIC_BUTTON_PASSPHRASE if not given.
        #[arg(long)]
        passphrase_file: Option<PathBuf>,
        /// This device's label [default: host name].
        #[arg(long)]
        label: Option<String>,
    },
    /// Make this device a Practice Admin using the recovery kit (asks the
    /// running agent). Use it when every admin computer is lost.
    Recover {
        /// The recovery kit file.
        #[arg(long)]
        recovery_kit: PathBuf,
        /// File holding the passphrase. Read from PANIC_BUTTON_PASSPHRASE if not
        /// given.
        #[arg(long)]
        passphrase_file: Option<PathBuf>,
    },
    /// Show the practice-wide settings, or change them (admin; asks the
    /// running agent, which signs and distributes the change).
    Settings {
        /// Hold time, 1000 to 3000 ms.
        #[arg(long)]
        hold_ms: Option<u32>,
        /// silent, soft or alarm_ramp.
        #[arg(long)]
        cue_mode: Option<String>,
        /// A, B or C.
        #[arg(long)]
        sound_pattern: Option<String>,
        /// silent or audible.
        #[arg(long)]
        drill_mode: Option<String>,
        /// For example "Mon-Fri 08:00-18:30".
        #[arg(long)]
        opening_hours: Option<String>,
        /// Relay URL (repeat for several); replaces the practice's list.
        #[arg(long)]
        relay: Vec<String>,
        /// Remove the practice's relay URLs.
        #[arg(long, conflicts_with = "relay")]
        clear_relays: bool,
        /// Weekly test alarm, like "Tue 07:45" (local time), or "off".
        #[arg(long)]
        drill_schedule: Option<String>,
        /// The device that runs the weekly test alarm (device id).
        #[arg(long)]
        drill_coordinator: Option<String>,
    },
}

fn passphrase(file: &Option<PathBuf>) -> anyhow::Result<String> {
    let p = match file {
        Some(f) => std::fs::read_to_string(panic_button_agent::paths::expand(f))
            .with_context(|| format!("reading {}", f.display()))?,
        None => std::env::var("PANIC_BUTTON_PASSPHRASE")
            .context("give --passphrase-file or set PANIC_BUTTON_PASSPHRASE")?,
    };
    let p = p.trim_end_matches(['\n', '\r']).to_owned();
    check_passphrase(&p)?;
    Ok(p)
}

pub fn run(cli: &Cli, c: &PracticeCommand) -> anyhow::Result<()> {
    if let PracticeCommand::Settings {
        hold_ms,
        cue_mode,
        sound_pattern,
        drill_mode,
        opening_hours,
        relay,
        clear_relays,
        drill_schedule,
        drill_coordinator,
    } = c
    {
        let mut p = serde_json::Map::new();
        if let Some(h) = hold_ms {
            p.insert("hold_ms".into(), json!(h));
        }
        if let Some(v) = cue_mode {
            p.insert("cue_mode".into(), json!(v));
        }
        if let Some(v) = sound_pattern {
            p.insert("sound_pattern".into(), json!(v));
        }
        if let Some(v) = drill_mode {
            p.insert("drill_mode".into(), json!(v));
        }
        if let Some(v) = opening_hours {
            p.insert("opening_hours".into(), json!(v));
        }
        if let Some(v) = drill_schedule {
            let v = (!v.eq_ignore_ascii_case("off")).then_some(v);
            p.insert("drill_schedule".into(), json!(v));
        }
        if let Some(v) = drill_coordinator {
            p.insert("drill_coordinator".into(), json!(v));
        }
        if *clear_relays {
            p.insert("relay_urls".into(), serde_json::Value::Null);
        } else if !relay.is_empty() {
            p.insert("relay_urls".into(), json!(relay));
        }
        return crate::ctl::call_and_emit(
            cli,
            if p.is_empty() {
                "practice.settings"
            } else {
                "practice.set_settings"
            },
            serde_json::Value::Object(p),
        );
    }
    if let PracticeCommand::Recover {
        recovery_kit,
        passphrase_file,
    } = c
    {
        let pass = passphrase(passphrase_file)?;
        eprintln!("Opening the recovery kit (this takes a few seconds)...");
        let kit = panic_button_agent::paths::expand(recovery_kit);
        return crate::ctl::call_and_emit(
            cli,
            "admin.recover",
            json!({"kit_path": kit.display().to_string(), "passphrase": pass}),
        );
    }
    match c {
        PracticeCommand::Settings { .. } | PracticeCommand::Recover { .. } => Ok(()),
        PracticeCommand::Init {
            name,
            recovery_kit,
            passphrase_file,
            label,
        } => {
            let pass = passphrase(passphrase_file)?;
            let label = label.clone().unwrap_or_else(|| "admin".into());
            eprintln!("Deriving the recovery kit key (this takes a few seconds)...");
            let kit = panic_button_agent::paths::expand(recovery_kit);
            let p = panic_button_agent::setup::init_practice(
                &cli.data_dir(),
                name,
                &label,
                &pass,
                &kit,
            )?;
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
