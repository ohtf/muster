// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Panic Button relay: a blind, multi-tenant forwarder of sealed envelopes.

#![forbid(unsafe_code)]

pub mod config;
pub mod persist;
pub mod server;
pub mod state;
