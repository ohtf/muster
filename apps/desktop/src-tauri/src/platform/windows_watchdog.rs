// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The Windows watchdog (`spec/05-desktop-agent.md`, Session and lifecycle):
//! a per-user Scheduled Task that runs `panic-button-desktop.exe
//! --ensure-running` every 5 minutes, so Panic Button is back within 5
//! minutes if it crashes or is ended. If it is running, the new process
//! hands over to it and exits, and nothing is shown.
//!
//! Task names are shared by every account on the computer, so each task is
//! named after its user. The MSI deletes them all on uninstall
//! (`packaging/windows/panic-button.wxs`).

use std::os::windows::process::CommandExt;
use std::process::Command;

/// Start the process without a console window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The MSI's uninstall step deletes tasks whose names start with this.
pub const TASK_PREFIX: &str = "Panic Button watchdog";

/// Characters Task Scheduler does not allow in a task name become `_`.
fn task_name(user: &str) -> String {
    let user: String = user
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{TASK_PREFIX} ({})", user.trim())
}

/// Creates or updates this user's watchdog task. `/F` replaces an existing
/// task, so the program path stays right after an upgrade.
pub fn register() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
    let name = task_name(&user);
    let run = format!("\"{}\" --ensure-running", exe.display());
    let out = Command::new("schtasks.exe")
        .args([
            "/Create", "/TN", &name, "/TR", &run, "/SC", "MINUTE", "/MO", "5", "/F",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) if o.status.success() => tracing::info!("watchdog task \"{name}\" registered"),
        Ok(o) => tracing::warn!(
            "could not register the watchdog task: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => tracing::warn!("could not run schtasks: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_names_are_per_user_and_safe() {
        assert_eq!(task_name("jsmith"), "Panic Button watchdog (jsmith)");
        assert_eq!(
            task_name("DOMAIN\\j:smith*"),
            "Panic Button watchdog (DOMAIN_j_smith_)"
        );
    }
}
