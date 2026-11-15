// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The four windows: the floating button, the panel beside it (sender
//! panel and drill toast), the receiver alert, and the main window. All are
//! created at start so each keeps its state; most stay hidden until needed.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tauri::window::Color;
use tauri::{
    AppHandle, Manager, PhysicalPosition, UserAttentionType, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::placement::{self, Monitor};

/// Logical sizes.
pub const BUTTON: f64 = 76.0;
const PANEL: (f64, f64) = (300.0, 176.0);
const ALERT: (f64, f64) = (560.0, 420.0);
const MAIN: (f64, f64) = (880.0, 620.0);
/// From the screen edge and the taskbar or dock.
const MARGIN: f64 = 24.0;
/// On Linux a window that is not resizable cannot be smaller than 200 x 200
/// (tauri-apps/tao#561), so the button and panel are resizable there; their
/// size limits keep them fixed anyway.
const SMALL_RESIZABLE: bool = cfg!(target_os = "linux");

fn page(view: &str) -> WebviewUrl {
    WebviewUrl::App(format!("index.html?view={view}").into())
}

/// Bottom-right of the primary monitor's work area, in physical pixels.
fn default_position(app: &AppHandle) -> Option<(i32, i32)> {
    let m = app.primary_monitor().ok().flatten()?;
    let s = m.scale_factor();
    let wa = m.work_area();
    let side = ((BUTTON + MARGIN) * s) as i32;
    Some((
        wa.position.x + wa.size.width as i32 - side,
        wa.position.y + wa.size.height as i32 - side,
    ))
}

fn monitors(app: &AppHandle) -> Vec<Monitor> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| Monitor {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
        })
        .collect()
}

/// Remembers where the button was moved to, per monitor layout, and puts it
/// back (`placement`). Does nothing on Wayland, where the compositor places
/// windows.
pub struct Placer {
    store: Mutex<placement::Store>,
    /// Moves before this instant are our own, not the person's.
    quiet_until: Mutex<Instant>,
    /// The layout the button was last placed for.
    layout: Mutex<String>,
    pending_save: AtomicU64,
    enabled: bool,
}

impl Placer {
    pub fn new(data_dir: &Path, wayland: bool) -> Self {
        Self {
            store: Mutex::new(placement::Store::load(data_dir)),
            quiet_until: Mutex::new(Instant::now()),
            layout: Mutex::new(String::new()),
            pending_save: AtomicU64::new(0),
            enabled: !wayland,
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Puts the button at its saved place for the current monitor layout, or
/// the default corner.
pub fn place_button(app: &AppHandle) {
    let (Some(p), Some(b)) = (app.try_state::<Placer>(), window(app, "button")) else {
        return;
    };
    let Some(default) = default_position(app) else {
        return;
    };
    let mons = monitors(app);
    let key = placement::layout_key(&mons);
    let pos = if p.enabled {
        placement::choose(lock(&p.store).get(&key), &mons, BUTTON, default)
    } else {
        default
    };
    *lock(&p.layout) = key;
    *lock(&p.quiet_until) = Instant::now() + Duration::from_secs(1);
    let _ = b.set_position(PhysicalPosition::new(pos.0, pos.1));
}

/// The button window moved. Unless we moved it, remember the place for this
/// monitor layout, writing the file once the moves stop.
pub fn button_moved(app: &AppHandle, pos: PhysicalPosition<i32>) {
    let Some(p) = app.try_state::<Placer>() else {
        return;
    };
    if !p.enabled || Instant::now() < *lock(&p.quiet_until) {
        return;
    }
    let key = placement::layout_key(&monitors(app));
    if !lock(&p.store).set(&key, (pos.x, pos.y)) {
        return;
    }
    *lock(&p.layout) = key;
    let n = p.pending_save.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Some(p) = app.try_state::<Placer>()
            && p.pending_save.load(Ordering::SeqCst) == n
        {
            lock(&p.store).save();
        }
    });
}

/// Every few seconds: if the monitors changed (a laptop docked or
/// undocked, a screen unplugged) or the button is no longer wholly on a
/// screen, place it again.
pub async fn watch_button(app: AppHandle) {
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    loop {
        tick.tick().await;
        let (Some(p), Some(b)) = (app.try_state::<Placer>(), window(&app, "button")) else {
            continue;
        };
        if !p.enabled {
            return;
        }
        let mons = monitors(&app);
        if mons.is_empty() {
            continue;
        }
        let key = placement::layout_key(&mons);
        let changed = *lock(&p.layout) != key;
        let lost = b
            .outer_position()
            .is_ok_and(|pos| !placement::visible((pos.x, pos.y), &mons, BUTTON));
        if changed || lost {
            tracing::info!(changed, lost, "placing the button again");
            place_button(&app);
        }
    }
}

pub fn create_all(app: &AppHandle, first_run: bool) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "button", page("button"))
        .title("Panic Button button")
        .inner_size(BUTTON, BUTTON)
        .min_inner_size(BUTTON, BUTTON)
        .max_inner_size(BUTTON, BUTTON)
        .background_color(Color(0, 0, 0, 0))
        .resizable(SMALL_RESIZABLE)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(!first_run)
        .build()?;
    place_button(app);
    WebviewWindowBuilder::new(app, "panel", page("panel"))
        .title("Panic Button panel")
        .inner_size(PANEL.0, PANEL.1)
        .min_inner_size(PANEL.0, PANEL.1)
        .max_inner_size(PANEL.0, PANEL.1)
        .resizable(SMALL_RESIZABLE)
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
        .title("Panic Button")
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
