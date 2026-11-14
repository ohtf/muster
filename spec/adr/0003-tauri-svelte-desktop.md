# ADR 0003: Tauri 2 with a Svelte 5 frontend for desktop

- Status: Accepted (2026-11-14)

## Context

The app needs a small floating always-on-top window, a topmost alert window, a tray icon, global shortcuts and an admin UI, on three desktop OSes, with a small footprint, packaged as an MSI.

## Decision

Tauri 2 (system webview: WebView2, WKWebView, WebKitGTK) with a Svelte 5 + TypeScript + Vite frontend. The frontend is a thin view. All decisions are made in Rust.

## Alternatives considered

- **Electron:** larger footprint, bundles Chromium; no advantage here.
- **Native per platform (WinUI, AppKit, GTK):** best fidelity, but three UI codebases.
- **egui/iced:** single Rust codebase, but weaker accessibility and system integration (tray, non-activating windows) at present.
- **Flutter desktop:** good UI, but adds Dart and FFI to a Rust core.

## Consequences

- WebView2 is a runtime dependency on Windows (normally present; the MSI bootstraps it).
- Some platform behaviours (non-activating windows, audio endpoint control, session detection) need small native modules in Rust.
- Tauri 2 mobile support is an option for phase 3 but is not committed to.
