// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The session this app runs in: active, locked, or a remote session
//! (`spec/05-desktop-agent.md`, Session and lifecycle). In a remote session
//! the core stops taking part unless `allow_remote_session` is set; a locked
//! screen keeps the alarm working and is reported in heartbeats.
//!
//! Windows asks the system (`platform::windows_session`). Linux asks logind
//! for the graphical session (`loginctl show-session auto`), and treats an
//! xrdp session as remote. Elsewhere, or if nothing can be read, the session
//! counts as active.

use panic_button::proto::SessionState;

#[cfg(windows)]
pub async fn read() -> SessionState {
    tokio::task::spawn_blocking(crate::platform::windows_session::read)
        .await
        .unwrap_or(SessionState::Active)
}

#[cfg(target_os = "linux")]
pub async fn read() -> SessionState {
    if std::env::var_os("XRDP_SESSION").is_some() {
        return SessionState::RemoteSessionDisabled;
    }
    let out = tokio::process::Command::new("loginctl")
        .args(["show-session", "auto", "-p", "LockedHint", "-p", "Remote"])
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(std::time::Duration::from_secs(3), out).await {
        Ok(Ok(o)) if o.status.success() => parse_loginctl(&String::from_utf8_lossy(&o.stdout)),
        _ => SessionState::Active,
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub async fn read() -> SessionState {
    SessionState::Active
}

/// `LockedHint=yes` / `Remote=yes` lines.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_loginctl(s: &str) -> SessionState {
    let yes = |k: &str| s.lines().any(|l| l.trim() == format!("{k}=yes"));
    if yes("Remote") {
        SessionState::RemoteSessionDisabled
    } else if yes("LockedHint") {
        SessionState::Locked
    } else {
        SessionState::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loginctl() {
        assert_eq!(
            parse_loginctl("Remote=no\nLockedHint=no\n"),
            SessionState::Active
        );
        assert_eq!(
            parse_loginctl("Remote=no\nLockedHint=yes\n"),
            SessionState::Locked
        );
        assert_eq!(
            parse_loginctl("Remote=yes\nLockedHint=yes\n"),
            SessionState::RemoteSessionDisabled
        );
        assert_eq!(parse_loginctl(""), SessionState::Active);
    }
}
