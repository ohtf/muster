// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! The four windows: the floating button, the panel beside it (sender
//! panel and drill toast), the receiver alert, and the main window. All are
//! created at start so each keeps its state; most stay hidden until needed.

use tauri::window::Color;
use tauri::{
    AppHandle, Manager, PhysicalPosition, UserAttentionType, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

/// Logical sizes.
pub const BUTTON: f64 = 76.0;
const PANEL: (f64, f64) = (300.0, 176.0);
const ALERT: (f64, f64) = (560.0, 420.0);
const MAIN: (f64, f64) = (880.0, 620.0);
/// From the screen edge and the taskbar or dock.
const MARGIN: f64 = 24.0;

fn page(view: &str) -> WebviewUrl {
    WebviewUrl::App(format!("index.html?view={view}").into())
}

/// Bottom-right of the primary monitor's work area, in physical pixels.
fn button_position(app: &AppHandle) -> Option<PhysicalPosition<i32>> {
    let m = app.primary_monitor().ok().flatten()?;
    let s = m.scale_factor();
    let wa = m.work_area();
    let side = ((BUTTON + MARGIN) * s) as i32;
    Some(PhysicalPosition::new(
        wa.position.x + wa.size.width as i32 - side,
        wa.position.y + wa.size.height as i32 - side,
    ))
}

pub fn create_all(app: &AppHandle, first_run: bool) -> tauri::Result<()> {
    let button = WebviewWindowBuilder::new(app, "button", page("button"))
        .title("Muster button")
        .inner_size(BUTTON, BUTTON)
        .min_inner_size(BUTTON, BUTTON)
        .max_inner_size(BUTTON, BUTTON)
        .background_color(Color(0, 0, 0, 0))
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(!first_run)
        .build()?;
    if let Some(p) = button_position(app) {
        let _ = button.set_position(p);
    }
    WebviewWindowBuilder::new(app, "panel", page("panel"))
        .title("Muster panel")
        .inner_size(PANEL.0, PANEL.1)
        .min_inner_size(PANEL.0, PANEL.1)
        .max_inner_size(PANEL.0, PANEL.1)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    WebviewWindowBuilder::new(app, "alert", page("alert"))
        .title("Help needed")
        .inner_size(ALERT.0, ALERT.1)
        .min_inner_size(420.0, 320.0)
        .always_on_top(true)
        .minimizable(false)
        .closable(false)
        .focused(false)
        .center()
        .visible(false)
        .build()?;
    WebviewWindowBuilder::new(app, "main", page("main"))
        .title("Muster")
        .inner_size(MAIN.0, MAIN.1)
        .min_inner_size(640.0, 480.0)
        .center()
        .visible(first_run)
        .build()?;
    Ok(())
}

fn window(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    app.get_webview_window(label)
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = window(app, "main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn show_button(app: &AppHandle) {
    if let Some(w) = window(app, "button") {
        let _ = w.show();
    }
}

/// The panel sits just above and to the left of the button.
pub fn show_panel(app: &AppHandle) {
    let Some(p) = window(app, "panel") else {
        return;
    };
    if let Some(b) = window(app, "button")
        && let (Ok(bp), Ok(s)) = (b.outer_position(), b.scale_factor())
    {
        let x = bp.x - ((PANEL.0 + 8.0) * s) as i32 + (BUTTON * s) as i32;
        let y = bp.y - ((PANEL.1 + 8.0) * s) as i32;
        let _ = p.set_position(PhysicalPosition::new(x.max(0), y.max(0)));
    }
    let _ = p.show();
}

/// Shown without taking focus where the platform allows; the taskbar or
/// dock is flashed instead (`spec/05-desktop-agent.md`, Receiver alert).
pub fn show_alert(app: &AppHandle) {
    if let Some(w) = window(app, "alert") {
        let _ = w.show();
        let _ = w.set_always_on_top(true);
        let _ = w.request_user_attention(Some(UserAttentionType::Critical));
    }
}
