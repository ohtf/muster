// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! JSON views for the control API. Stable field names; no secrets.

use muster_core::io::{AlarmPhase, AlarmView, LocalAck};
use muster_core::peers::PeerClass;
use muster_core::{Core, Mono};
use muster_proto::StatusLevel;
use serde_json::{Value, json};

fn level(l: StatusLevel) -> &'static str {
    match l {
        StatusLevel::Green => "green",
        StatusLevel::Amber => "amber",
        StatusLevel::Red => "red",
        StatusLevel::Other(_) => "unknown",
    }
}

pub fn status(core: &Core, _now: Mono, _wall: u64) -> Value {
    let st = core.status();
    json!({
        "device": core.device_id().to_hex(),
        "device_short": core.device_id().short(),
        "enrolled": core.is_enrolled(),
        "practice": core.practice().map(|p| p.name.clone()),
        "epoch": core.current_epoch(),
        "level": st.map(|s| level(s.level)),
        "summary": st.map(|s| s.summary()),
        "reasons": st.map(|s| s.reasons.iter().map(|r| r.text()).collect::<Vec<_>>()).unwrap_or_default(),
        "direct": st.map(|s| s.direct),
        "via_relay": st.map(|s| s.via_relay),
    })
}

fn class(c: PeerClass) -> &'static str {
    match c {
        PeerClass::LanGroup => "lan",
        PeerClass::UnicastOnly => "unicast",
        PeerClass::RelayOnly => "relay",
        PeerClass::Indirect => "indirect",
        PeerClass::Stale => "stale",
    }
}

pub fn peers(core: &Core, now: Mono, wall: u64) -> Value {
    let member = |d| core.trust().is_some_and(|t| t.is_member(d, wall));
    Value::Array(
        core.peers()
            .iter()
            .map(|p| {
                let hb = p.heartbeat.as_ref();
                json!({
                    "device": p.device_id.to_hex(),
                    "device_short": p.device_id.short(),
                    "member": member(&p.device_id),
                    "label": hb.map(|h| h.label.clone()),
                    "room": hb.map(|h| h.room.clone()),
                    "user": hb.map(|h| h.user_display.clone()),
                    "path": class(p.class(now)),
                    "last_heard_ms": p.last_heard().map(|t| now.since(t)),
                    "app_version": hb.map(|h| h.app_version.clone()),
                    "audio_ok": p.audio().map(|a| a.ok()),
                })
            })
            .collect(),
    )
}

pub fn alarm(v: &AlarmView) -> Value {
    let (state, by, at) = match &v.state {
        AlarmPhase::Open => ("open", None, None),
        AlarmPhase::Resolved { by, at } => ("resolved", Some(by.clone()), Some(*at)),
        AlarmPhase::Cancelled { at } => ("cancelled", None, Some(*at)),
        AlarmPhase::Expired => ("expired", None, None),
    };
    json!({
        "alarm_id": v.alarm_id.to_hex(),
        "origin": v.origin.to_hex(),
        "own": v.own,
        "drill": v.drill,
        "room": v.room,
        "user": v.user_display,
        "device_label": v.device_label,
        "raised_at": v.raised_at,
        "state": state,
        "resolved_by": by,
        "closed_at": at,
        "delivered": v.delivered,
        "coming": v.coming,
        "seen": v.seen,
        "local_ack": v.local_ack.map(|a| match a {
            LocalAck::Seen => "seen",
            LocalAck::Responding => "responding",
        }),
    })
}

pub fn diagnostics(core: &Core, audit_failures: u64) -> Value {
    let drops: serde_json::Map<String, Value> = core
        .diagnostics()
        .iter()
        .map(|(k, v)| (format!("{k:?}"), json!(v)))
        .collect();
    json!({"dropped": drops, "audit_write_failures": audit_failures})
}
