// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Platform code that needs operating system calls (`unsafe`, with
//! `// SAFETY:` comments), kept here so it is easy to find and review.

#[cfg(windows)]
pub mod windows_audio;
