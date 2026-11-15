// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Random world generation and the property runs used by the property
//! tests and the `explore` example (`spec/07-testing.md`, Properties).

#![allow(clippy::needless_range_loop)]

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddrV4};

use panic_button_core::io::UserAction;
use panic_button_proto::{
    AlarmEvent, AlarmKind, AlarmRaised, EventDetail, EventKind, MsgId, Payload, Source,
};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

use crate::props::*;
use crate::world::{NodeSpec, Segment, UserModel, World, WorldEvent, WorldSpec};

pub fn max_nodes() -> usize {
    std::env::var("PANIC_BUTTON_SIM_MAX_NODES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60)
}

/// Shrinkable top-level parameters; details come from `seed`.
#[derive(Debug, Clone)]
pub struct Params {
    pub nodes: usize,
    pub segments: Vec<Segment>,
    pub relays: usize,
    pub events: usize,
    pub skew: bool,
    pub seed: u64,
}

impl Params {
    /// Parses the `Debug` form proptest prints for a failing case, so the
    /// `explore` example can re-run it.
    pub fn parse_debug(s: &str) -> Option<Params> {
        let tokens: Vec<&str> = s
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|t| !t.is_empty())
            .collect();
        let mut it = tokens.iter().peekable();
        let mut p = Params {
            nodes: 0,
            segments: Vec::new(),
            relays: 0,
            events: 0,
            skew: false,
            seed: 0,
        };
        let mut seg: Option<Segment> = None;
        while let Some(t) = it.next() {
            match *t {
                "Segment" => {
                    if let Some(s) = seg.take() {
                        p.segments.push(s);
                    }
                    seg = Some(Segment::default());
                }
                "multicast_ok" => seg.as_mut()?.multicast_ok = it.next().copied()? == "true",
                "broadcast_ok" => seg.as_mut()?.broadcast_ok = it.next().copied()? == "true",
                "client_isolation" => {
                    seg.as_mut()?.client_isolation = it.next().copied()? == "true"
                }
                "loss_permille" => {
                    seg.as_mut()?.loss_permille = it.next().copied()?.parse().ok()?
                }
                "latency_min" => seg.as_mut()?.latency_min = it.next().copied()?.parse().ok()?,
                "latency_max" => seg.as_mut()?.latency_max = it.next().copied()?.parse().ok()?,
                "dup_permille" => seg.as_mut()?.dup_permille = it.next().copied()?.parse().ok()?,
                "nodes" => p.nodes = it.next().copied()?.parse().ok()?,
                "relays" => {
                    if let Some(s) = seg.take() {
                        p.segments.push(s);
                    }
                    p.relays = it.next().copied()?.parse().ok()?
                }
                "events" => p.events = it.next().copied()?.parse().ok()?,
                "skew" => p.skew = it.next().copied()? == "true",
                "seed" => p.seed = it.next().copied()?.parse().ok()?,
                _ => {}
            }
        }
        Some(p)
    }
}

pub fn build(p: &Params) -> WorldSpec {
    let mut r = ChaCha20Rng::seed_from_u64(p.seed);
    let mut pick = |n: u64| if n == 0 { 0 } else { r.next_u64() % n };
    let ns = p.segments.len();
    let mut route_udp = WorldSpec::no_routes(ns);
    let mut route_tcp = WorldSpec::no_routes(ns);
    for a in 0..ns {
        for b in a + 1..ns {
            let u = pick(2) == 1;
            let t = u && pick(3) > 0;
            route_udp[a][b] = u;
            route_udp[b][a] = u;
            route_tcp[a][b] = t;
            route_tcp[b][a] = t;
        }
    }
    let mut nodes = Vec::new();
    for i in 0..p.nodes {
        let s0 = pick(ns as u64) as usize;
        let mut segments = vec![s0];
        if ns > 1 && pick(10) == 0 {
            let s1 = pick(ns as u64) as usize;
            if s1 != s0 {
                segments.push(s1);
            }
        }
        let relays = (0..p.relays).filter(|_| pick(10) < 6).collect();
        let seeds = if pick(10) < 3 && p.nodes > 1 {
            vec![(i + 1 + pick(p.nodes as u64 - 1) as usize) % p.nodes]
        } else {
            vec![]
        };
        let user = match pick(3) {
            0 => UserModel::Respond {
                after: 1_000 + pick(3_000),
            },
            1 => UserModel::Seen {
                after: 1_000 + pick(3_000),
            },
            _ => UserModel::Never,
        };
        let clock_offset = if p.skew {
            pick(20 * 60_000 + 1) as i64 - 10 * 60_000
        } else {
            0
        };
        nodes.push(NodeSpec {
            segments,
            clock_offset,
            seeds,
            relays,
            user,
            start_at: pick(3_000),
            ..Default::default()
        });
    }
    let mut events = Vec::new();
    for _ in 0..p.events {
        let t = 5_000 + pick(15_000);
        let n = pick(p.nodes as u64) as usize;
        let e = match pick(6) {
            0 => {
                let s = nodes[n].segments[0];
                events.push((
                    t + 1 + pick(5_000),
                    WorldEvent::Link {
                        node: n,
                        segment: s,
                        up: true,
                    },
                ));
                WorldEvent::Link {
                    node: n,
                    segment: s,
                    up: false,
                }
            }
            1 => {
                events.push((t + 1 + pick(5_000), WorldEvent::Wake(n)));
                WorldEvent::Sleep(n)
            }
            2 => {
                events.push((t + 1 + pick(5_000), WorldEvent::Restart(n)));
                WorldEvent::Crash(n)
            }
            3 if p.relays > 0 => WorldEvent::RelayUp(pick(p.relays as u64) as usize, pick(2) == 0),
            4 => {
                // Late enough that every device has heard every neighbour's
                // heartbeat under the initial flags (the model assumes it).
                let s = pick(ns as u64) as usize;
                events.push((
                    18_000 + pick(2_000),
                    WorldEvent::SegmentFlags {
                        segment: s,
                        multicast_ok: pick(2) == 0,
                        broadcast_ok: pick(2) == 0,
                        client_isolation: pick(8) == 0,
                    },
                ));
                continue;
            }
            99 => {
                let s = pick(ns as u64) as usize;
                WorldEvent::SegmentFlags {
                    segment: s,
                    multicast_ok: pick(2) == 0,
                    broadcast_ok: pick(2) == 0,
                    client_isolation: pick(8) == 0,
                }
            }
            _ => WorldEvent::Sleep(n),
        };
        events.push((t, e));
    }
    // At most one flag change per segment: the reachability model knows
    // only the initial and final flags.
    let mut changed = BTreeSet::new();
    events.retain(|(_, e)| match e {
        WorldEvent::SegmentFlags { segment, .. } => changed.insert(*segment),
        _ => true,
    });
    // Anything asleep is woken by 22 s, so the topology is stable from then.
    for i in 0..p.nodes {
        events.push((21_000, WorldEvent::Wake(i)));
    }
    WorldSpec {
        seed: p.seed,
        segments: p.segments.clone(),
        route_udp,
        route_tcp,
        nodes,
        relays: p.relays,
        relay_latency: (10, 60),
        practices: 1,
        admins: vec![],
        events,
    }
}

pub const STABLE_FROM: u64 = 22_000;
/// Topology is stable from STABLE_FROM. P8 needs PEER_STALE + 1 s more, and
/// re-learning addresses after a change takes up to PEER_STALE plus a
/// heartbeat round.
pub const T0: u64 = 90_000;
pub const T1: u64 = 140_000;
pub const END: u64 = 200_000;

fn pick_alive(w: &World, seed: u64) -> Option<usize> {
    let alive: Vec<usize> = (0..w.node_count()).filter(|i| w.is_up(*i)).collect();
    if alive.is_empty() {
        None
    } else {
        Some(alive[(seed as usize) % alive.len()])
    }
}

/// The main run: P1, P3, P4, P5, P8, P10, P11 (no forgeries), P12.
pub fn run_world(p: &Params) -> Result<(), String> {
    run_world_with(p, |_| {}).1
}

/// As [`run_world`], returning the world for inspection.
pub fn run_world_with(p: &Params, setup: impl FnOnce(&mut World)) -> (World, Result<(), String>) {
    let spec = build(p);
    let mut w = World::new(spec);
    setup(&mut w);
    let r = run_world_inner(p, &mut w);
    (w, r)
}

fn run_world_inner(p: &Params, w: &mut World) -> Result<(), String> {
    // A drill from the admin (P12).
    w.user(30_000, 0, UserAction::RunDrill);
    w.run_until(T0);
    let g0 = graph(&*w);
    p8_status_honesty(&*w, &g0, STABLE_FROM)?;
    let Some(o1) = pick_alive(&*w, p.seed) else {
        return Ok(());
    };
    let a1 = w.raise_now(o1).ok_or("raise 1 failed")?;
    w.run_until(T0 + 10_000);
    p1_delivery(&*w, &g0, &a1, o1, T0)?;
    w.run_until(T1);
    let mut cancelled = BTreeSet::new();
    if let Some(o2) = pick_alive(&*w, p.seed / 7 + 1) {
        let a2 = w.raise_now(o2).ok_or("raise 2 failed")?;
        if p.seed.is_multiple_of(2) {
            w.user(T1 + 4_000, o2, UserAction::Cancel { alarm_id: a2 });
            cancelled.insert(a2);
        } else if let Some(r) = pick_alive(&*w, p.seed / 3) {
            w.user(T1 + 6_000, r, UserAction::Resolve { alarm_id: a2 });
        }
    }
    w.user(T1 + 2_000, o1, UserAction::Resolve { alarm_id: a1 });
    w.run_until(END);
    let g = graph(&*w);
    p3_no_duplicates(&*w)?;
    p4_convergence(&*w, &g)?;
    if !p.skew {
        p5_no_stale(&*w)?;
    }
    p8_status_honesty(&*w, &g, STABLE_FROM)?;
    p10_sender_honesty(&*w)?;
    p11_cancel_authority(&*w, &cancelled)?;
    p12_drill_isolation(&*w)?;
    p2_authenticity(&*w)?;
    Ok(())
}

/// P2 and P11 under attack: garbage, replays, bit flips, cross-practice
/// traffic, forgeries by a non-member holding the group key, and a
/// Cancelled event from a member who is not the origin.
pub fn run_attack(p: &Params) -> Result<(), String> {
    let mut spec = build(p);
    let n = spec.nodes.len();
    // A second practice shares the first segment.
    spec.practices = 2;
    for k in 0..2 {
        spec.nodes.push(NodeSpec {
            practice: 1,
            ..Default::default()
        });
        let _ = k;
    }
    let mut w = World::new(spec);
    w.capture = true;
    w.run_until(T0);
    let Some(o1) = pick_alive(&w, p.seed) else {
        return Ok(());
    };
    let a1 = w.raise_now(o1).ok_or("raise failed")?;
    // The other practice raises too.
    let _ = w.raise_now(n);
    w.run_until(T0 + 5_000);
    w.capture = false;
    let attacker = SocketAddrV4::new(Ipv4Addr::new(10, 0, 9, 9), 47474);
    let mut r = ChaCha20Rng::seed_from_u64(p.seed ^ 0xa77ac);
    let captured = w.captured.clone();
    let mut t = w.now + 1_000;
    for (i, (_, b)) in captured.iter().enumerate().take(200) {
        // Replay, and a bit-flipped copy.
        w.inject_segment(t, 0, attacker, b.clone());
        let mut flipped = b.clone();
        let pos = (r.next_u64() as usize) % flipped.len().max(1);
        if let Some(x) = flipped.get_mut(pos) {
            *x ^= 1 << (i % 8);
        }
        w.inject_segment(t, 0, attacker, flipped);
        t += 10;
    }
    for _ in 0..50 {
        let len = (r.next_u64() % 300) as usize;
        let mut junk = vec![0u8; len];
        r.fill_bytes(&mut junk);
        w.inject_segment(t, 0, attacker, junk);
    }
    // Forged alarm: a stranger who has the group key but no cert.
    let stranger = panic_button_crypto::DeviceKeys::from_secrets([0x55; 32], [0x66; 32]);
    let fake = MsgId([0xfa; 16]);
    let forged = forge(
        &w,
        0,
        &stranger,
        fake,
        Payload::AlarmRaised(AlarmRaised {
            alarm_id: fake,
            kind: AlarmKind::Help,
            drill: false,
            room: "Fake".into(),
            user_display: "Fake".into(),
            device_label: "Fake".into(),
            source: Source::Button,
        }),
    );
    w.inject_segment(t + 100, 0, attacker, forged);
    // P11: a member who is not the origin sends Cancelled.
    let insider = (o1 + 1) % n;
    let cancel_id = MsgId([0xca; 16]);
    let cancel = forge(
        &w,
        0,
        w.keys(insider),
        cancel_id,
        Payload::AlarmEvent(AlarmEvent {
            alarm_id: a1,
            event: EventKind::Cancelled,
            user_display: "Insider".into(),
            detail: EventDetail::Cancelled { reason: 0 },
        }),
    );
    for s in 0..w.spec.segments.len() {
        w.inject_segment(t + 200, s, attacker, cancel.clone());
    }
    w.run_until(t + 30_000);
    p2_authenticity(&w)?;
    p3_no_duplicates(&w)?;
    p11_cancel_authority(&w, &BTreeSet::new())?;
    if w.trace.iter().any(|e| {
        matches!(&e.ev, crate::world::TraceEv::Ui(panic_button_core::io::UiEvent::AlarmShown { alarm, .. }) if alarm.alarm_id == fake)
    }) {
        return Err("P2: forged alarm shown".into());
    }
    Ok(())
}

/// P6: after revocation and rotation reach a node, it rejects the revoked
/// device, and the revoked device never holds the new epoch key.
pub fn run_revocation(p: &Params) -> Result<(), String> {
    let mut spec = build(p);
    if spec.nodes.len() < 3 {
        return Ok(());
    }
    spec.events
        .retain(|(_, e)| !matches!(e, WorldEvent::Crash(0) | WorldEvent::Sleep(0)));
    let mut w = World::new(spec);
    w.run_until(T0);
    let n = w.node_count();
    let victim = 1 + (p.seed as usize) % (n - 1);
    let vid = w.device_id(victim);
    w.act_now(
        0,
        UserAction::Revoke {
            device: vid,
            cascade: false,
            reason: "test".into(),
        },
    );
    w.run_until(T0 + 30_000);
    // The victim raises an alarm after the revocation.
    let a = w.raise_now(victim);
    w.run_until(T0 + 45_000);
    for i in 0..n {
        if i == victim {
            continue;
        }
        let Some(c) = w.core(i) else { continue };
        let knows = c.trust().is_some_and(|t| t.is_device_revoked(&vid));
        if knows
            && let Some(a) = a
            && w.shown(&a).contains_key(&i)
        {
            return Err(format!(
                "P6: node {i} showed an alarm from the revoked device"
            ));
        }
    }
    if w.core(victim).and_then(|c| c.current_epoch()).unwrap_or(0) >= 1 {
        return Err("P6: revoked device holds the new epoch key".into());
    }
    Ok(())
}
