// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Properties P1 to P12 over random worlds (`spec/07-testing.md`).
//!
//! Case count follows `PROPTEST_CASES` (8 by default; CI's property job
//! runs 256 in release, nightly 1,000). The
//! largest world size follows `PANIC_BUTTON_SIM_MAX_NODES` (default 60). Failing
//! cases are saved by proptest in `properties.proptest-regressions` and
//! must be committed as regression tests. To explore many seeds quickly
//! and get a failing case as code, use
//! `cargo run --release -p panic-button-sim --example explore`.

use panic_button_sim::worlds::*;
use panic_button_sim::*;
use proptest::prelude::*;

/// `PROPTEST_CASES` if set (CI's property job uses 256, nightly 1,000);
/// otherwise a quick 8 so `cargo test` stays fast on every platform.
fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

fn arb_segment() -> impl Strategy<Value = Segment> {
    (
        prop::bool::weighted(0.8),
        prop::bool::weighted(0.8),
        prop::bool::weighted(0.15),
        prop_oneof![3 => Just(0u32), 1 => 0u32..=300],
        1u64..10,
        0u64..20,
        prop_oneof![3 => Just(0u32), 1 => 0u32..50],
    )
        .prop_map(|(m, b, iso, loss, lmin, lx, dup)| Segment {
            multicast_ok: m,
            broadcast_ok: b,
            client_isolation: iso,
            loss_permille: loss,
            latency_min: lmin,
            latency_max: lmin + lx,
            dup_permille: dup,
        })
}

fn arb_params(skew: bool) -> impl Strategy<Value = Params> {
    (
        2usize..=max_nodes(),
        prop::collection::vec(arb_segment(), 1..=4),
        0usize..=2,
        0usize..=6,
        any::<u64>(),
    )
        .prop_map(move |(nodes, segments, relays, events, seed)| Params {
            nodes,
            segments,
            relays,
            events,
            skew,
            seed,
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: cases(),
        max_shrink_iters: 200,
        ..ProptestConfig::default()
    })]

    #[test]
    fn p1_p3_p4_p5_p8_p10_p11_p12_random_worlds(p in arb_params(false)) {
        if let Err(e) = run_world(&p) {
            prop_assert!(false, "{e}");
        }
    }

    #[test]
    fn p9_clock_skew_up_to_10_minutes(p in arb_params(true)) {
        if let Err(e) = run_world(&p) {
            prop_assert!(false, "{e}");
        }
    }

    #[test]
    fn p2_p11_attacks(p in arb_params(false)) {
        if let Err(e) = run_attack(&p) {
            prop_assert!(false, "{e}");
        }
    }

    #[test]
    fn p6_revocation(p in arb_params(false)) {
        if let Err(e) = run_revocation(&p) {
            prop_assert!(false, "{e}");
        }
    }

    /// P7: total first transmissions per alarm are at most 10 x N on a
    /// fully connected LAN.
    #[test]
    fn p7_bounded_traffic(n in 2usize..=max_nodes(), seed in any::<u64>()) {
        let mut s = WorldSpec::flat(n, seed);
        for (i, node) in s.nodes.iter_mut().enumerate() {
            node.start_at = (i as u64 * 97) % 3_000;
        }
        let mut w = World::new(s);
        w.run_until(40_000);
        let base = w.first_packets_between(30_000, 40_000);
        w.raise_now(0).ok_or_else(|| TestCaseError::fail("raise"))?;
        w.run_until(50_000);
        let during = w.first_packets_between(40_000, 50_000).saturating_sub(base);
        prop_assert!(during <= 10 * n as u64, "{during} first transmissions for {n} nodes");
    }
}
