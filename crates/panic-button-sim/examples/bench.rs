// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Rough simulator throughput check: `cargo run --release -p panic-button-sim --example bench -- 30`.

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let t = std::time::Instant::now();
    let mut w = panic_button_sim::World::new(panic_button_sim::WorldSpec::flat(n, 1));
    w.classify_traffic = std::env::var_os("CLASSIFY").is_some();
    println!("setup {:?}", t.elapsed());
    let t = std::time::Instant::now();
    w.run_until(
        std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(40_000),
    );
    println!(
        "40 s idle: {:?}, packets {} (udp {} tcp {} relay {})",
        t.elapsed(),
        w.counters.packets,
        w.counters.udp,
        w.counters.tcp,
        w.counters.relay
    );
    let c0 = w.counters.clone();
    for (k, v) in &w.counters.by_type {
        println!("  idle {k}: {v}");
    }
    let t = std::time::Instant::now();
    w.raise_now(0);
    w.run_for(10_000);
    println!(
        "10 s alarm: {:?}, packets {} (udp {} tcp {} relay {})",
        t.elapsed(),
        w.counters.packets - c0.packets,
        w.counters.udp - c0.udp,
        w.counters.tcp - c0.tcp,
        w.counters.relay - c0.relay
    );
    for (k, v) in &w.counters.by_type {
        let before = c0.by_type.get(k).copied().unwrap_or(0);
        if *v > before {
            println!("  alarm {k}: {}", v - before);
        }
    }
}
