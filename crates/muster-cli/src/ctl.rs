// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! `muster ctl ...`: commands to a running agent over its control socket.

use anyhow::Context;
use clap::Subcommand;
use serde_json::{Value, json};

use crate::{Cli, emit};

#[derive(Debug, Subcommand)]
pub enum CtlCommand {
    /// Status light and reasons.
    Status,
    /// Known peers and how they are reached.
    Peers,
    /// Known alarms.
    Alarms,
    /// Raise an alarm now (only if the agent allows it).
    Raise,
    /// Wait for the next alert to appear.
    WaitAlarm {
        /// For example 3s, 500ms, 2m.
        #[arg(long, default_value = "10s")]
        timeout: String,
    },
    /// Respond "I'm coming".
    Respond { alarm_id: String },
    /// "Seen - can't come".
    Seen { alarm_id: String },
    /// Mark an alarm resolved.
    Resolve { alarm_id: String },
    /// Cancel your own alarm as sent in error.
    Cancel { alarm_id: String },
    /// Run a drill now (admin).
    Drill,
    /// Remove a device (admin). `--compromised` also removes everything it enrolled.
    Revoke {
        device: String,
        #[arg(long)]
        compromised: bool,
        #[arg(long, default_value = "")]
        reason: String,
    },
    /// Dropped-message counters.
    Diagnostics,
}

/// Parses durations like `3s`, `500ms`, `2m`.
pub fn parse_duration_ms(s: &str) -> anyhow::Result<u64> {
    let s = s.trim();
    let (num, mult) = if let Some(n) = s.strip_suffix("ms") {
        (n, 1)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1000)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60_000)
    } else {
        (s, 1000)
    };
    let n: u64 = num
        .trim()
        .parse()
        .with_context(|| format!("bad duration {s:?}"))?;
    Ok(n * mult)
}

fn status_text(v: &Value) -> String {
    let mut s = format!(
        "{}: {}",
        v["level"].as_str().unwrap_or("unknown").to_uppercase(),
        v["summary"].as_str().unwrap_or("")
    );
    for r in v["reasons"].as_array().into_iter().flatten() {
        s.push_str(&format!("\n  - {}", r.as_str().unwrap_or("")));
    }
    s
}

fn alarm_line(a: &Value) -> String {
    format!(
        "{}  {}  {}  {} ({})  delivered {}  coming {}",
        a["alarm_id"].as_str().unwrap_or(""),
        a["state"].as_str().unwrap_or(""),
        a["room"].as_str().unwrap_or(""),
        a["user"].as_str().unwrap_or(""),
        a["device_label"].as_str().unwrap_or(""),
        a["delivered"],
        a["coming"].as_array().map(|c| c.len()).unwrap_or(0)
    )
}

pub fn run(cli: &Cli, c: &CtlCommand) -> anyhow::Result<()> {
    let (method, params): (&str, Value) = match c {
        CtlCommand::Status => ("status", Value::Null),
        CtlCommand::Peers => ("peers", Value::Null),
        CtlCommand::Alarms => ("alarms", Value::Null),
        CtlCommand::Raise => ("raise", Value::Null),
        CtlCommand::WaitAlarm { timeout } => (
            "wait_alarm",
            json!({"timeout_ms": parse_duration_ms(timeout)?}),
        ),
        CtlCommand::Respond { alarm_id } => ("respond", json!({"alarm_id": alarm_id})),
        CtlCommand::Seen { alarm_id } => ("seen", json!({"alarm_id": alarm_id})),
        CtlCommand::Resolve { alarm_id } => ("resolve", json!({"alarm_id": alarm_id})),
        CtlCommand::Cancel { alarm_id } => ("cancel", json!({"alarm_id": alarm_id})),
        CtlCommand::Drill => ("drill.run", Value::Null),
        CtlCommand::Revoke {
            device,
            compromised,
            reason,
        } => (
            "admin.revoke",
            json!({"device": device, "cascade": compromised, "reason": reason}),
        ),
        CtlCommand::Diagnostics => ("diagnostics", Value::Null),
    };
    let sock = cli.control_socket();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    #[cfg(unix)]
    let v = rt
        .block_on(muster_agent::control::call(&sock, method, params))
        .with_context(|| format!("calling the agent at {}", sock.display()))?;
    #[cfg(not(unix))]
    let v: Value = {
        let _ = (rt, params);
        anyhow::bail!("the control API is not yet available on this platform");
    };
    emit(cli.format, &v, |v| match c {
        CtlCommand::Status => status_text(v),
        CtlCommand::Alarms => v
            .as_array()
            .map(|a| a.iter().map(alarm_line).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default(),
        CtlCommand::Raise | CtlCommand::WaitAlarm { .. } => alarm_line(v),
        CtlCommand::Peers => v
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| {
                        format!(
                            "{}  {}  {}  {}",
                            p["device_short"].as_str().unwrap_or(""),
                            p["path"].as_str().unwrap_or(""),
                            p["label"].as_str().unwrap_or("-"),
                            p["room"].as_str().unwrap_or("-")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        _ => v.to_string(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration_ms("3s").expect("3s"), 3000);
        assert_eq!(parse_duration_ms("500ms").expect("ms"), 500);
        assert_eq!(parse_duration_ms("2m").expect("m"), 120_000);
        assert!(parse_duration_ms("x").is_err());
    }
}
