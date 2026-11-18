// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Panic Button agent runtime: wires core, network, store and platform; control API.

pub mod config;
pub mod control;
pub mod paths;
pub mod runtime;
pub mod schedule;
pub mod setup;
pub mod views;

pub use runtime::{AgentError, AgentHandle, AgentOptions, run, start};
