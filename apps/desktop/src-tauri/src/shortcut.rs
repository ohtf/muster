// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The global keyboard shortcut (FR-ACT-006). Holding it is the same as
//! holding the button: Pressed sends `hold.start` with source `hotkey` and
//! Released sends `hold.end`, so the core decides when the hold counts.
//! Repeated Pressed events (key auto-repeat) are ignored.
//!
//! Works on Windows, macOS and X11. A Wayland session cannot report global
//! key presses to an app without the GlobalShortcuts portal, which is not
//! supported yet; the shortcut is then reported as unavailable (Settings).

use std::str::FromStr;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

/// `alarm.hotkey` default (`spec/08-deployment.md`).
pub const DEFAULT: &str = "Ctrl+Shift+F12";

/// What Settings shows about the shortcut.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Info {
    /// As configured, for example "Ctrl+Shift+F12"; empty when turned off.
    pub keys: String,
    pub active: bool,
    /// Why it is not active, in words for staff.
    pub problem: Option<String>,
}

#[derive(Debug)]
pub struct State {
    info: Mutex<Info>,
    held: Mutex<bool>,
}

impl State {
    pub fn info(&self) -> Info {
        lock(&self.info).clone()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The configured shortcut: `None` when turned off (`""` or `"none"`).
pub fn parse(configured: Option<&str>) -> Result<Option<Shortcut>, String> {
    let s = configured.unwrap_or(DEFAULT).trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    Shortcut::from_str(s)
        .map(Some)
        .map_err(|e| format!("\"{s}\" is not a keyboard shortcut Panic Button understands ({e})"))
}

/// A session where global shortcuts cannot work: Wayland, even when the app
/// runs under XWayland (X11 key grabs only see keys sent to X11 windows).
fn wayland_session() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Registers the plugin and the shortcut. Never fails the app: a shortcut
/// that cannot be used is logged and shown in Settings.
pub fn setup(app: &AppHandle, configured: Option<&str>) {
    let keys = configured.unwrap_or(DEFAULT).trim().to_owned();
    let info = |active, problem: Option<String>| Info {
        keys: keys.clone(),
        active,
        problem,
    };
    let result = match parse(configured) {
        Ok(None) => info(false, Some("Turned off by this computer's IT configuration.".into())),
        Ok(Some(_)) if wayland_session() => info(
            false,
            Some(
                "Keyboard shortcuts for other apps do not work on this desktop (Wayland). Use the on-screen button."
                    .into(),
            ),
        ),
        Ok(Some(sc)) => match register(app, sc) {
            Ok(()) => info(true, None),
            Err(e) => {
                tracing::warn!("keyboard shortcut {keys}: {e}");
                info(
                    false,
                    Some(format!(
                        "{keys} could not be set up, perhaps because another program uses it. Ask IT to choose another."
                    )),
                )
            }
        },
        Err(e) => {
            tracing::warn!("keyboard shortcut: {e}");
            info(false, Some(format!("{e}. Ask IT to correct it.")))
        }
    };
    tracing::info!(keys = %result.keys, active = result.active, "keyboard shortcut");
    app.manage(State {
        info: Mutex::new(result),
        held: Mutex::new(false),
    });
}

fn register(app: &AppHandle, sc: Shortcut) -> Result<(), String> {
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(on_event)
            .build(),
    )
    .map_err(|e| e.to_string())?;
    app.global_shortcut()
        .register(sc)
        .map_err(|e| e.to_string())
}

fn on_event(app: &AppHandle, _sc: &Shortcut, e: ShortcutEvent) {
    let Some(st) = app.try_state::<State>() else {
        return;
    };
    let method = {
        let mut held = lock(&st.held);
        match (e.state(), *held) {
            (ShortcutState::Pressed, false) => {
                *held = true;
                "hold.start"
            }
            (ShortcutState::Released, true) => {
                *held = false;
                "hold.end"
            }
            _ => return,
        }
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(h) = crate::agent_handle(&app).await
            && let Err(e) = h.call(method, json!({ "source": "hotkey" })).await
        {
            tracing::debug!("shortcut {method}: {}", e.message);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_off() {
        assert!(parse(None).expect("default").is_some());
        assert_eq!(
            parse(None).expect("default"),
            parse(Some("ctrl+shift+f12")).expect("ctrl+shift+f12")
        );
        assert_eq!(parse(Some("")).expect(""), None);
        assert_eq!(parse(Some(" None ")).expect(" None "), None);
    }

    #[test]
    fn other_shortcuts() {
        assert!(parse(Some("Alt+Shift+P")).expect("Alt+Shift+P").is_some());
        assert!(
            parse(Some("CmdOrCtrl+Shift+F12"))
                .expect("CmdOrCtrl+Shift+F12")
                .is_some()
        );
        assert!(parse(Some("Ctrl+Banana")).is_err());
        assert!(parse(Some("Ctrl+")).is_err());
    }
}
