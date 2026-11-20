// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Run by the MSI to write its properties into the machine config.
    if args.first().map(String::as_str) == Some("--install-settings") {
        std::process::exit(panic_button_desktop::install_settings(&args[1..]));
    }
    panic_button_desktop::run()
}
