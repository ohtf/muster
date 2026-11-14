// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Muster deterministic network simulator and property tests.

#![forbid(unsafe_code)]

pub mod practice;
pub mod props;
pub mod world;
pub mod worlds;

pub use world::{NodeSpec, Segment, UserModel, World, WorldEvent, WorldSpec};
