// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The core's only source of randomness: a ChaCha20 RNG seeded at
//! construction (`spec/02-architecture.md`, Rules). The runtime seeds it
//! from the OS RNG and reseeds it via [`crate::core::Core::reseed`] when
//! [`CoreRng::needs_reseed`] says so; the simulator seeds it
//! deterministically.

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

/// Reseed after this many draws.
pub const RESEED_AFTER: u64 = 1 << 20;

pub struct CoreRng {
    rng: ChaCha20Rng,
    draws: u64,
}

impl std::fmt::Debug for CoreRng {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CoreRng(draws={})", self.draws)
    }
}

impl CoreRng {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        CoreRng {
            rng: ChaCha20Rng::from_seed(seed),
            draws: 0,
        }
    }

    pub fn reseed(&mut self, seed: [u8; 32]) {
        self.rng = ChaCha20Rng::from_seed(seed);
        self.draws = 0;
    }

    pub fn needs_reseed(&self) -> bool {
        self.draws >= RESEED_AFTER
    }

    pub fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut b = [0u8; N];
        self.rng.fill_bytes(&mut b);
        self.draws += 1;
        b
    }

    /// Uniform in `0..=max`.
    pub fn below_incl(&mut self, max: u64) -> u64 {
        self.draws += 1;
        if max == u64::MAX {
            return self.rng.next_u64();
        }
        self.rng.next_u64() % (max + 1)
    }
}
