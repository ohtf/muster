// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Runs the property worlds over a range of seeds and prints the first
//! failure as code that rebuilds it.
//!
//! `cargo run --release -p panic-button-sim --example explore -- world 0 200 30`
//! (kind: world | skew | attack | revocation; first seed; count; max nodes)

use panic_button_sim::world::Segment;
use panic_button_sim::worlds::*;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

fn params(seed: u64, max_nodes: usize, skew: bool) -> Params {
    let mut r = ChaCha20Rng::seed_from_u64(seed ^ 0x5eed);
    let mut pick = |n: u64| r.next_u64() % n;
    let nseg = 1 + pick(4) as usize;
    let segments = (0..nseg)
        .map(|_| {
            let lmin = 1 + pick(9);
            Segment {
                multicast_ok: pick(5) > 0,
                broadcast_ok: pick(5) > 0,
                client_isolation: pick(7) == 0,
                loss_permille: if pick(4) == 0 { pick(301) as u32 } else { 0 },
                latency_min: lmin,
                latency_max: lmin + pick(20),
                dup_permille: if pick(4) == 0 { pick(50) as u32 } else { 0 },
            }
        })
        .collect();
    Params {
        nodes: 2 + pick(max_nodes as u64 - 1) as usize,
        segments,
        relays: pick(3) as usize,
        events: pick(7) as usize,
        skew,
        seed: r.next_u64(),
    }
}

fn detail(seed: u64, max: usize, node: usize) {
    use panic_button_sim::world::World;
    let p = match std::env::var("CASE") {
        Ok(c) => Params::parse_debug(&c).expect("CASE parses"),
        Err(_) => params(seed, max, std::env::var_os("SKEW").is_some()),
    };
    let spec = build(&p);
    for (i, n) in spec.nodes.iter().enumerate() {
        println!(
            "node {i}: segs {:?} seeds {:?} relays {:?} start {}",
            n.segments, n.seeds, n.relays, n.start_at
        );
    }
    for (i, s) in spec.segments.iter().enumerate() {
        println!("seg {i}: {s:?}");
    }
    println!(
        "udp {:?}\ntcp {:?}\nevents {:?}",
        spec.route_udp, spec.route_tcp, spec.events
    );
    let mut w = World::new(spec);
    w.run_until(T0);
    let g = panic_button_sim::props::graph(&w);
    let c = w.core(node).expect("core");
    let now = panic_button::core::Mono(w.now);
    println!("node {node} status {:?}", c.status());
    println!(
        "out degree {} in degree {}",
        g.out_degree(node),
        g.in_degree(node)
    );
    println!("edges out of {node}: {:?}", g.edges_of(node));
    if let Some(o) = std::env::args()
        .nth(5)
        .and_then(|s| s.parse::<usize>().ok())
    {
        println!("bounds from {o}: {:?}", g.bounds_from(o));
        let mut w2 = World::new(build(&p));
        w2.classify_traffic = true;
        w2.watch = std::env::var("WATCH")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
            .unwrap_or_default();
        w2.run_until(T0);
        w2.watched.clear();
        {
            let c = w2.core(8).expect("8");
            let p4 = c.peers().get(&w2.device_id(4)).cloned();
            println!(
                "8's peer 4 at T0: {:?}",
                p4.map(|p| (
                    p.class(panic_button::core::Mono(w2.now)),
                    p.heard,
                    p.udp_addr
                ))
            );
        }
        let id = w2.raise_now(o).expect("raise");
        w2.run_until(T0 + 10_000);
        println!("received: {:?}", w2.received(&id));
        for i in 0..w2.node_count() {
            if let Some(c) = w2.core(i)
                && !c.diagnostics().is_empty()
            {
                println!("diag {i}: {:?}", c.diagnostics());
            }
        }
        let pairs: Vec<(usize, usize)> = std::env::var("PAIRS")
            .ok()
            .map(|v| {
                v.split(',')
                    .filter_map(|x| {
                        let mut it = x.split(':');
                        Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (a, b) in pairs {
            let c = w2.core(a).expect("core");
            let d = w2.device_id(b);
            println!(
                "{a} sees {b}: member {:?} class {:?}",
                c.trust().map(|t| t.is_member(&d, w2.wall_of(a))),
                c.peers()
                    .get(&d)
                    .map(|p| p.class(panic_button::core::Mono(w2.now)))
            );
        }
        for (t, n, d) in w2.watched.iter().filter(|(t, _, _)| *t < T0 + 3000) {
            println!("  {t} node {n}: {d}");
        }
        for (k, v) in &w2.counters.by_type {
            if k.contains("Alarm") {
                println!("  {k}: {v}");
            }
        }
        for m in 0..w.node_count() {
            println!("  edges of {m}: {:?}", g.edges_of(m));
        }
    }
    for m in 0..w.node_count() {
        if g.edges_of(m).iter().any(|(x, _)| *x == node) {
            println!(
                "  edge {m} -> {node}: {:?}",
                g.edges_of(m).iter().find(|(x, _)| *x == node)
            );
        }
    }
    for peer in c.peers().iter() {
        let who = w.node_of(&peer.device_id);
        println!(
            "  peer {:?}: class {:?} udp {:?} tcp {} heard {:?} member {:?}",
            who,
            peer.class(now),
            peer.udp_addr,
            peer.tcp_port,
            peer.heard,
            c.trust()
                .map(|t| t.is_member(&peer.device_id, w.wall_of(node)))
        );
    }
}

fn p4(seed: u64, max: usize) {
    let p = match std::env::var("CASE") {
        Ok(c) => Params::parse_debug(&c).expect("CASE parses"),
        Err(_) => params(seed, max, std::env::var_os("SKEW").is_some()),
    };
    println!("{p:?}");
    let watch: Vec<usize> = std::env::var("WATCH")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let (w, r) = run_world_with(&p, |w| {
        w.watch = watch;
        w.classify_traffic = true;
    });
    println!("{r:?}");
    let from: u64 = std::env::var("FROM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let to: u64 = std::env::var("TO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for (t, n, d) in &w.watched {
        if *t >= from && *t < to {
            println!("  {t} node {n}: {d}");
        }
    }
    for (id, (o, t)) in &w.raised {
        println!("raised {id} by {o} at {t}: received {:?}", w.received(id));
    }
    if let Ok(ns) = std::env::var("VIEWS") {
        for n in ns.split(',').filter_map(|x| x.parse::<usize>().ok()) {
            println!(
                "digests {n} ({}): {:?}",
                w.device_id(n).short(),
                w.core(n).map(|c| c.digests(w.wall_of(n)).0)
            );
            for v in w
                .core(n)
                .map(|c| c.alarm_views(w.wall_of(n)))
                .unwrap_or_default()
            {
                println!(
                    "view {n} {}: {:?} delivered {} coming {:?} seen {:?}",
                    v.alarm_id, v.state, v.delivered, v.coming, v.seen
                );
            }
        }
    }
    if let Ok(n) = std::env::var("NODE").map(|v| v.parse::<usize>().unwrap_or(0)) {
        let c = w.core(n).expect("core");
        let now = panic_button::core::Mono(w.now);
        println!(
            "node {n} status {:?} diag {:?}",
            c.status(),
            c.diagnostics()
        );
        for peer in c.peers().iter() {
            println!(
                "  peer {:?}: class {:?} udp {:?} tcp {} heard {:?} member {:?}",
                w.node_of(&peer.device_id),
                peer.class(now),
                peer.udp_addr,
                peer.tcp_port,
                peer.heard,
                c.trust()
                    .map(|t| t.is_member(&peer.device_id, w.wall_of(n)))
            );
        }
    }
    let spec = &w.spec;
    for (i, n) in spec.nodes.iter().enumerate() {
        println!(
            "node {i}: segs {:?} seeds {:?} relays {:?} up {} offset {}",
            n.segments,
            n.seeds,
            n.relays,
            w.is_up(i),
            n.clock_offset
        );
    }
    for (i, s) in spec.segments.iter().enumerate() {
        println!("seg {i}: {s:?} now {:?}", w.segments[i]);
    }
    println!("udp {:?}\ntcp {:?}", spec.route_udp, spec.route_tcp);
    let ids: Vec<_> = w.raised.keys().copied().collect();
    for id in ids {
        let counts: Vec<String> = (0..w.node_count())
            .map(|i| {
                w.core(i)
                    .and_then(|c| {
                        c.alarm_views(w.wall_of(i))
                            .into_iter()
                            .find(|v| v.alarm_id == id)
                    })
                    .map(|v| format!("{i}:{}", v.delivered))
                    .unwrap_or_else(|| format!("{i}:-"))
            })
            .collect();
        println!("alarm {id}: {}", counts.join(" "));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.get(1).map(String::as_str) == Some("p6") {
        let p = match std::env::var("CASE") {
            Ok(c) => Params::parse_debug(&c).expect("CASE parses"),
            Err(_) => params(
                a[2].parse().expect("seed"),
                a[3].parse().expect("max"),
                false,
            ),
        };
        let spec = build(&p);
        let n = spec.nodes.len();
        let mut w = panic_button_sim::World::new(spec);
        w.watch = (0..n).collect();
        w.classify_traffic = true;
        let victim = 1 + (p.seed as usize) % (n - 1);
        println!("victim {victim}");
        println!("{:?}", run_revocation(&p));
        w.run_until(T0);
        let vid = w.device_id(victim);
        w.act_now(
            0,
            panic_button::core::io::UserAction::Revoke {
                device: vid,
                cascade: false,
                reason: "test".into(),
            },
        );
        w.run_until(T0 + 45_000);
        for (t, node, d) in &w.watched {
            if d.contains(&format!("to Some({victim})"))
                || (*node == 0 && d.contains("KeyShare") && *t < T0 + 100)
            {
                println!("  {t} node {node}: {d}");
            }
        }
        println!(
            "victim epoch {:?}",
            w.core(victim).and_then(|c| c.current_epoch())
        );
        return;
    }
    if a.get(1).map(String::as_str) == Some("p4") {
        return p4(a[2].parse().expect("seed"), a[3].parse().expect("max"));
    }
    if a.get(1).map(String::as_str) == Some("detail") {
        let seed = a[2].parse().expect("seed");
        let max = a[3].parse().expect("max");
        let node = a[4].parse().expect("node");
        return detail(seed, max, node);
    }
    let kind = a.get(1).map(String::as_str).unwrap_or("world");
    let from: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let count: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
    let max: usize = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(30);
    let mut failures = 0;
    for s in from..from + count {
        let p = params(s, max, kind == "skew");
        let r = match kind {
            "attack" => run_attack(&p),
            "revocation" => run_revocation(&p),
            _ => run_world(&p),
        };
        if let Err(e) = r {
            failures += 1;
            println!("seed {s}: {e}");
            if failures == 1 {
                println!(
                    "{}",
                    format!("{p:?}").replace("segments: [", "segments: vec![")
                );
            }
        }
    }
    println!("{failures} failures in {count} runs");
}
