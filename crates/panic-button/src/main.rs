// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

use clap::Parser;

#[allow(unsafe_code)]
fn reset_sigpipe() {
    #[cfg(unix)]
    // SAFETY: called once at start-up, before any threads exist; restoring the
    // default SIGPIPE disposition lets output pipe cleanly into `head`.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

fn main() -> anyhow::Result<()> {
    reset_sigpipe();
    panic_button::cli::run(panic_button::cli::Cli::parse())
}
