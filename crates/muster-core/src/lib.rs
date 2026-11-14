// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Muster sans-IO core state machine: peers, alarms, hold logic, status and sync.

#![forbid(unsafe_code)]

pub mod alarm;
pub mod bootstrap;
pub mod config;
pub mod digest;
pub mod drill;
mod engine;
pub mod hold;
pub mod io;
pub mod peers;
pub mod persist;
pub mod rng;
pub mod status;
pub mod time;

pub use engine::{Core, Identity};
pub use io::{Input, Output};
pub use time::{Mono, WallMs};
