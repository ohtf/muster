// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Muster relay: a blind, multi-tenant forwarder of sealed envelopes.

#![forbid(unsafe_code)]

pub mod config;
pub mod server;
pub mod state;
