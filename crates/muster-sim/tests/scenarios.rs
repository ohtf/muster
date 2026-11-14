// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Named scenarios S1 to S17 (`spec/07-testing.md`, Named scenarios).
//! Deterministic; always run.

#![allow(clippy::needless_range_loop)]

use std::net::{Ipv4Addr, SocketAddrV4};

use muster_core::io::{AlarmPhase, LocalAck, SenderPhase, StatusReason, UiEvent, UserAction};
use muster_core::time::MINUTE;
use muster_proto::{AudioStatus, MsgId, StatusLevel};
use muster_sim::*;

const WARMUP: u64 = 40_000;

fn status_of(w: &World, i: usize) -> (Option<StatusLevel>, Vec<StatusReason>) {
    let s = w.core(i).and_then(|c| c.status());
    (
        s.map(|s| s.level),
        s.map(|s| s.reasons.clone()).unwrap_or_default(),
    )
}

fn assert_status(w: &World, nodes: impl IntoIterator<Item = usize>, level: StatusLevel) {
    for i in nodes {
        assert_eq!(
            status_of(w, i).0,
            Some(level),
            "node {i}: {:?}",
            status_of(w, i)
        );
    }
}

fn delivered_within(
    w: &World,
    alarm: &MsgId,
    t0: u64,
    bound: u64,
    nodes: impl IntoIterator<Item = usize>,
) {
    let shown = w.shown(alarm);
    for i in nodes {
        let t = shown.get(&i).copied();
        assert!(
            t.is_some_and(|t| t <= t0 + bound),
            "node {i} shown at {t:?}, raised at {t0}, bound {bound}"
        );
    }
}

fn sender_delivered(w: &World, node: usize, alarm: &MsgId) -> usize {
    w.ui_events(node)
        .filter_map(|(_, e)| match e {
            UiEvent::SenderPanel { alarm: a, .. } if a.alarm_id == *alarm => Some(a.delivered),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

fn others(n: usize, except: &[usize]) -> Vec<usize> {
    (0..n).filter(|i| !except.contains(i)).collect()
}

fn view_of(w: &World, node: usize, alarm: &MsgId) -> Option<muster_core::io::AlarmView> {
    w.core(node)?
        .alarm_views(w.wall_of(node))
        .into_iter()
        .find(|v| v.alarm_id == *alarm)
}

fn two_vlans(a: usize, b: usize, dual: Option<usize>) -> WorldSpec {
    let mut s = WorldSpec::flat(a + b, 4);
    s.segments = vec![Segment::default(), Segment::default()];
    s.route_udp = WorldSpec::no_routes(2);
    s.route_tcp = WorldSpec::no_routes(2);
    for i in a..a + b {
        s.nodes[i].segments = vec![1];
    }
    if let Some(d) = dual {
        s.nodes[d].segments = vec![0, 1];
    }
    s
}

#[test]
fn s1_flat_lan_30_nodes() {
    let mut w = World::new(WorldSpec::flat(30, 1));
    w.run_until(WARMUP);
    assert_status(&w, 0..30, StatusLevel::Green);
    let t0 = w.now;
    let id = w.raise_now(5).expect("raised");
    w.run_for(5_000);
    delivered_within(&w, &id, t0, 2_000, others(30, &[5]));
    // The sender never alerts for its own alarm, and is told it was delivered.
    assert!(!w.shown(&id).contains_key(&5));
    assert_eq!(sender_delivered(&w, 5, &id), 29);
}

#[test]
fn s2_multicast_blocked_broadcast_allowed() {
    let mut s = WorldSpec::flat(10, 2);
    s.segments[0].multicast_ok = false;
    let mut w = World::new(s);
    w.run_until(WARMUP);
    assert_status(&w, 0..10, StatusLevel::Green);
    let t0 = w.now;
    let id = w.raise_now(0).expect("raised");
    w.run_for(5_000);
    delivered_within(&w, &id, t0, 2_000, 1..10);
}

#[test]
fn s3_unicast_only_with_seed_peer() {
    let mut s = WorldSpec::flat(8, 3);
    s.segments[0].multicast_ok = false;
    s.segments[0].broadcast_ok = false;
    for i in 1..8 {
        s.nodes[i].seeds = vec![0];
    }
    let mut w = World::new(s);
    w.run_until(WARMUP);
    assert_status(&w, 0..8, StatusLevel::Green);
    let t0 = w.now;
    let id = w.raise_now(3).expect("raised");
    w.run_for(5_000);
    delivered_within(&w, &id, t0, 2_000, others(8, &[3]));
}

#[test]
fn s4_two_vlans_bridged_by_dual_homed_node() {
    let mut w = World::new(two_vlans(5, 5, Some(4)));
    w.run_until(WARMUP);
    let t0 = w.now;
    let id = w.raise_now(0).expect("raised");
    w.run_for(5_000);
    delivered_within(&w, &id, t0, 2_000, 1..10);
    let id = w.raise_now(9).expect("raised");
    let t1 = w.now;
    w.run_for(5_000);
    delivered_within(&w, &id, t1, 2_000, 0..9);
}

#[test]
fn s5_two_vlans_via_relay() {
    let mut s = two_vlans(5, 1, None);
    s.relays = 1;
    for n in &mut s.nodes {
        n.relays = vec![0];
    }
    let mut w = World::new(s);
    w.run_until(WARMUP);
    assert_status(&w, 0..5, StatusLevel::Green);
    let (lvl, reasons) = status_of(&w, 5);
    assert_eq!(lvl, Some(StatusLevel::Amber));
    assert!(reasons.contains(&StatusReason::OnlyViaRelay), "{reasons:?}");
    let t0 = w.now;
    let id = w.raise_now(0).expect("raised");
    w.run_for(6_000);
    delivered_within(&w, &id, t0, 5_000, 1..6);
    let id = w.raise_now(5).expect("raised");
    let t1 = w.now;
    w.run_for(6_000);
    delivered_within(&w, &id, t1, 5_000, 0..5);
    assert_eq!(sender_delivered(&w, 5, &id), 5);
}

fn wifi_isolation(relay: bool) -> WorldSpec {
    let mut s = WorldSpec::flat(7, 6);
    s.segments = vec![
        Segment::default(),
        Segment {
            client_isolation: true,
            ..Segment::default()
        },
    ];
    s.route_udp = WorldSpec::no_routes(2);
    s.route_tcp = WorldSpec::no_routes(2);
    for i in 4..7 {
        s.nodes[i].segments = vec![1];
    }
    if relay {
        s.relays = 1;
        for n in &mut s.nodes {
            n.relays = vec![0];
        }
    }
    s
}

#[test]
fn s6_wifi_isolation_with_relay() {
    let mut w = World::new(wifi_isolation(true));
    w.run_until(WARMUP);
    let t0 = w.now;
    let id = w.raise_now(0).expect("raised");
    w.run_for(6_000);
    delivered_within(&w, &id, t0, 5_000, 1..7);
    // A laptop can raise and reach the wired PCs.
    let id = w.raise_now(5).expect("raised");
    let t1 = w.now;
    w.run_for(6_000);
    delivered_within(&w, &id, t1, 5_000, [0, 1, 2, 3, 4, 6]);
}

#[test]
fn s7_wifi_isolation_without_relay_is_red() {
    let mut w = World::new(wifi_isolation(false));
    w.run_until(WARMUP);
    assert_status(&w, 0..4, StatusLevel::Green);
    for i in 4..7 {
        let (lvl, reasons) = status_of(&w, i);
        assert_eq!(lvl, Some(StatusLevel::Red), "laptop {i}");
        assert!(reasons.contains(&StatusReason::NoDevicesReachable));
    }
    // The laptop's sender panel says it could not deliver (P8, H03).
    let id = w.raise_now(5).expect("raised");
    w.run_for(16_000);
    let phases: Vec<SenderPhase> = w
        .ui_events(5)
        .filter_map(|(_, e)| match e {
            UiEvent::SenderPanel { alarm, phase } if alarm.alarm_id == id => Some(*phase),
            _ => None,
        })
        .collect();
    assert!(phases.contains(&SenderPhase::NotYetDelivered), "{phases:?}");
    assert_eq!(phases.last(), Some(&SenderPhase::CouldNotReach));
    assert!(w.shown(&id).is_empty());
}

#[test]
fn s8_asleep_through_alarm_sees_history_only() {
    let mut s = WorldSpec::flat(5, 8);
    s.nodes[1].user = UserModel::Respond { after: 2_000 };
    let mut w = World::new(s);
    w.run_until(WARMUP);
    w.at(w.now, WorldEvent::Sleep(4));
    w.run_for(100);
    let id = w.raise_now(0).expect("raised");
    w.run_for(10_000);
    w.act_now(1, UserAction::Resolve { alarm_id: id });
    w.run_for(10_000);
    w.at(w.now, WorldEvent::Wake(4));
    w.run_for(20_000);
    assert!(
        !w.shown(&id).contains_key(&4),
        "must not alert after resolution"
    );
    let history = w
        .ui_events(4)
        .any(|(_, e)| matches!(e, UiEvent::AlarmHistory(v) if v.alarm_id == id));
    assert!(history, "the alarm is in history");
    assert!(matches!(
        view_of(&w, 4, &id).map(|v| v.state),
        Some(AlarmPhase::Resolved { .. })
    ));
}

#[test]
fn s9_wakes_during_open_alarm_raised_10_minutes_ago() {
    let mut w = World::new(WorldSpec::flat(5, 9));
    w.run_until(WARMUP);
    w.at(w.now, WorldEvent::Sleep(4));
    w.run_for(100);
    let raised_wall = w.wall_of(0);
    let id = w.raise_now(0).expect("raised");
    w.run_for(10 * MINUTE);
    w.at(w.now, WorldEvent::Wake(4));
    let woke = w.now;
    w.run_for(10_000);
    let shown = w.shown(&id);
    assert!(
        shown.get(&4).is_some_and(|t| *t <= woke + 5_000),
        "{shown:?}"
    );
    let v = view_of(&w, 4, &id).expect("view");
    assert_eq!(v.raised_at, raised_wall);
    assert!(w.wall_of(4) - v.raised_at >= 10 * MINUTE);
}

#[test]
fn s10_two_practices_on_one_segment() {
    let mut s = WorldSpec::flat(8, 10);
    s.practices = 2;
    for i in 4..8 {
        s.nodes[i].practice = 1;
    }
    let mut w = World::new(s);
    w.run_until(WARMUP);
    assert_status(&w, 0..8, StatusLevel::Green);
    for i in 0..8 {
        let c = w.core(i).expect("core");
        let st = c.status().expect("status");
        assert_eq!(st.direct, 3, "node {i} sees only its own practice");
    }
    let id = w.raise_now(0).expect("raised");
    w.run_for(5_000);
    let shown = w.shown(&id);
    assert_eq!(shown.keys().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn s11_replays_never_re_alert() {
    let mut s = WorldSpec::flat(5, 11);
    for n in &mut s.nodes {
        n.user = UserModel::Seen { after: 3_000 };
    }
    let mut w = World::new(s);
    w.run_until(WARMUP);
    w.capture = true;
    let id = w.raise_now(0).expect("raised");
    w.run_for(1_000);
    w.capture = false;
    let raises: Vec<Vec<u8>> = w
        .captured
        .iter()
        .filter(|(_, b)| w.classify(b) == "AlarmRaised")
        .map(|(_, b)| b.clone())
        .collect();
    assert!(!raises.is_empty());
    let attacker = SocketAddrV4::new(Ipv4Addr::new(10, 0, 9, 9), 47474);
    let t = w.now + 5 * MINUTE;
    for b in &raises {
        w.inject_segment(t, 0, attacker, b.clone());
    }
    w.run_for(6 * MINUTE);
    w.at(w.now, WorldEvent::Crash(3));
    w.run_for(1_000);
    w.at(w.now, WorldEvent::Restart(3));
    w.run_for(MINUTE);
    let t = w.now + 2 * 60 * MINUTE - w.now % MINUTE;
    for b in &raises {
        w.inject_segment(t, 0, attacker, b.clone());
    }
    w.run_until(t + MINUTE);
    for i in 1..5 {
        let n = w
            .ui_events(i)
            .filter(|(_, e)| matches!(e, UiEvent::AlarmShown { alarm, resumed: false } if alarm.alarm_id == id))
            .count();
        assert_eq!(n, 1, "node {i} alerted {n} times");
    }
}

#[test]
fn s12_relay_down_mid_alarm() {
    let mut s = two_vlans(5, 1, None);
    s.relays = 1;
    for n in &mut s.nodes {
        n.relays = vec![0];
    }
    s.nodes[1].user = UserModel::Respond { after: 3_000 };
    let mut w = World::new(s);
    w.run_until(WARMUP);
    let t0 = w.now;
    let id = w.raise_now(0).expect("raised");
    w.at(t0 + 1_000, WorldEvent::RelayUp(0, false));
    w.run_for(5_000);
    // LAN delivery is unaffected.
    delivered_within(&w, &id, t0, 2_000, 1..5);
    assert!(view_of(&w, 5, &id).is_some_and(|v| v.coming.is_empty()));
    w.at(t0 + 20_000, WorldEvent::RelayUp(0, true));
    w.run_until(t0 + 30_000);
    let v = view_of(&w, 5, &id).expect("relay-only node has the alarm");
    assert_eq!(
        v.coming,
        vec!["User 1".to_string()],
        "events arrive after reconnect"
    );
}

fn s13_size() -> usize {
    if std::env::var_os("MUSTER_FULL_SCALE").is_some() {
        250
    } else {
        60
    }
}

#[test]
fn s13_large_flat_lan_all_respond() {
    let n = s13_size();
    let mut s = WorldSpec::flat(n, 13);
    for (i, node) in s.nodes.iter_mut().enumerate() {
        node.user = UserModel::Respond {
            after: 2_000 + (i as u64 * 37) % 3_000,
        };
        node.start_at = (i as u64 * 53) % 5_000;
    }
    let mut w = World::new(s);
    w.run_until(WARMUP);
    let base = w.first_packets_between(WARMUP - 10_000, WARMUP);
    let t0 = w.now;
    let id = w.raise_now(0).expect("raised");
    w.run_for(10_000);
    delivered_within(&w, &id, t0, 2_000, 1..n);
    let during = w.first_packets_between(t0, t0 + 10_000);
    let alarm_packets = during.saturating_sub(base);
    // P7: no more than 10 x N first transmissions (retransmissions are
    // allowed on top), with background heartbeat traffic subtracted.
    assert!(
        alarm_packets <= 10 * n as u64,
        "alarm traffic {alarm_packets} for {n} nodes (background {base}, total {})",
        w.packets_between(t0, t0 + 10_000)
    );
    let v = view_of(&w, 0, &id).expect("view");
    assert_eq!(v.delivered, n - 1);
    assert_eq!(v.coming.len(), n - 1);
}

#[test]
fn s14_revoke_laptop_during_open_alarm() {
    let mut s = WorldSpec::flat(6, 14);
    s.nodes[2].user = UserModel::Respond { after: 4_000 };
    let mut w = World::new(s);
    w.run_until(WARMUP);
    let t0 = w.now;
    let id = w.raise_now(1).expect("raised");
    w.run_for(1_000);
    let laptop = w.device_id(5);
    w.act_now(
        0,
        UserAction::Revoke {
            device: laptop,
            cascade: false,
            reason: "Laptop lost".into(),
        },
    );
    w.run_for(1_000);
    // The laptop tries to respond after revocation: everyone rejects it.
    w.act_now(5, UserAction::Respond { alarm_id: id });
    w.run_for(5_000);
    for i in 0..5 {
        let v = view_of(&w, i, &id).expect("view");
        assert_eq!(v.state, AlarmPhase::Open, "alarm continues at {i}");
        assert!(
            !v.coming.contains(&"User 5".to_string()),
            "node {i}: {:?}",
            v.coming
        );
        assert!(
            v.coming.contains(&"User 2".to_string()),
            "node {i}: {:?}",
            v.coming
        );
    }
    // After rotation the laptop cannot read new traffic.
    w.run_for(20_000);
    let e = w.core(0).and_then(|c| c.current_epoch());
    assert_eq!(e, Some(1));
    assert_eq!(w.core(5).and_then(|c| c.current_epoch()), Some(0));
    let id2 = w.raise_now(3).expect("raised");
    w.run_for(5_000);
    assert!(!w.received(&id2).contains_key(&5));
    delivered_within(&w, &id2, t0 + 27_000, 3_000, [0, 1, 2, 4]);
    assert_eq!(status_of(&w, 5).0, Some(StatusLevel::Red));
}

#[test]
fn s15_token_enrolment_with_stale_epoch() {
    let mut s = WorldSpec::flat(6, 15);
    s.nodes[5].start_at = 60_000;
    let mut w = World::new(s);
    w.run_until(WARMUP);
    let gone = w.device_id(4);
    w.act_now(
        0,
        UserAction::Revoke {
            device: gone,
            cascade: false,
            reason: "Left".into(),
        },
    );
    w.run_until(59_000);
    assert_eq!(w.core(1).and_then(|c| c.current_epoch()), Some(1));
    // Node 5 starts with the epoch-0 key from its token.
    w.run_until(60_000 + 20_000);
    assert_eq!(
        w.core(5).and_then(|c| c.current_epoch()),
        Some(1),
        "got the current key"
    );
    assert_eq!(
        status_of(&w, 5).0,
        Some(StatusLevel::Green),
        "{:?}",
        status_of(&w, 5)
    );
    let id = w.raise_now(5).expect("raised");
    let t0 = w.now;
    w.run_for(5_000);
    delivered_within(&w, &id, t0, 2_000, [0, 1, 2, 3]);
}

#[test]
fn s16_concurrent_rotations_converge() {
    let mut s = WorldSpec::flat(8, 16);
    s.admins = vec![1];
    let mut w = World::new(s);
    w.run_until(WARMUP);
    let a = w.device_id(6);
    let b = w.device_id(7);
    w.act_now(
        0,
        UserAction::Revoke {
            device: a,
            cascade: false,
            reason: String::new(),
        },
    );
    w.act_now(
        1,
        UserAction::Revoke {
            device: b,
            cascade: false,
            reason: String::new(),
        },
    );
    // Alarms keep flowing during the conflict.
    w.run_for(200);
    let id = w.raise_now(2).expect("raised");
    let t0 = w.now;
    w.run_for(5_000);
    delivered_within(&w, &id, t0, 2_000, [0, 1, 3, 4, 5]);
    w.run_for(60_000);
    let tags: Vec<_> = (0..6)
        .map(|i| {
            let c = w.core(i).expect("core");
            (
                c.current_epoch(),
                c.trust().map(|t| t.revocations().count()),
            )
        })
        .collect();
    assert!(tags.iter().all(|t| *t == tags[0]), "{tags:?}");
    assert_eq!(tags[0], (Some(1), Some(2)));
    let id = w.raise_now(3).expect("raised");
    let t1 = w.now;
    w.run_for(5_000);
    delivered_within(&w, &id, t1, 2_000, [0, 1, 2, 4, 5]);
}

#[test]
fn s17_muted_receiver_without_override() {
    let mut s = WorldSpec::flat(4, 17);
    s.nodes[2].audio = AudioStatus {
        output_present: true,
        muted: true,
        volume_pct: 80,
    };
    let mut w = World::new(s);
    w.run_until(WARMUP);
    let (lvl, reasons) = status_of(&w, 2);
    assert_eq!(lvl, Some(StatusLevel::Amber));
    assert!(reasons.contains(&StatusReason::AudioMuted));
    let id = w.raise_now(0).expect("raised");
    w.run_for(3_000);
    assert_eq!(
        sender_delivered(&w, 0, &id),
        3,
        "sender still gets the receipt"
    );
    let peer = w
        .core(0)
        .and_then(|c| c.peers().get(&w.device_id(2)).cloned());
    assert!(peer.and_then(|p| p.audio()).is_some_and(|a| a.muted));
    let _ = LocalAck::Seen;
}
