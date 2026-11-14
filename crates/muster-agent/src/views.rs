// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! JSON views for the control API. Stable field names; no secrets.

use muster_core::io::{AlarmPhase, AlarmView, LocalAck, SenderPhase, UiEvent};
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

pub fn status(core: &Core, _now: Mono, wall: u64) -> Value {
    let st = core.status();
    let me = core.device_id();
    json!({
        "device": me.to_hex(),
        "admin": core.trust().is_some_and(|t| t.is_admin(&me, wall)),
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

/// A UI event as JSON for the desktop webview: `{"type": ..., ...}`.
pub fn ui_event(e: &UiEvent) -> Value {
    match e {
        UiEvent::AlarmShown { alarm: a, resumed } => {
            json!({"type": "alarm_shown", "alarm": alarm(a), "resumed": resumed})
        }
        UiEvent::AlarmUpdated(a) => json!({"type": "alarm_updated", "alarm": alarm(a)}),
        UiEvent::AlarmClosed(a) => json!({"type": "alarm_closed", "alarm": alarm(a)}),
        UiEvent::AlarmHistory(a) => json!({"type": "alarm_history", "alarm": alarm(a)}),
        UiEvent::SenderPanel { alarm: a, phase } => json!({
            "type": "sender_panel",
            "alarm": alarm(a),
            "phase": match phase {
                SenderPhase::Requested => "requested",
                SenderPhase::Delivered => "delivered",
                SenderPhase::NotYetDelivered => "not_yet_delivered",
                SenderPhase::CouldNotReach => "could_not_reach",
            },
        }),
        UiEvent::DrillToast { alarm_id, audible } => {
            json!({"type": "drill_toast", "alarm_id": alarm_id.to_hex(), "audible": audible})
        }
        UiEvent::StatusChanged(st) => json!({
            "type": "status",
            "level": level(st.level),
            "summary": st.summary(),
            "reasons": st.reasons.iter().map(|r| r.text()).collect::<Vec<_>>(),
            "direct": st.direct,
            "via_relay": st.via_relay,
        }),
        UiEvent::HoldProgress { fraction_permille } => {
            json!({"type": "hold_progress", "fraction": f64::from(*fraction_permille) / 1000.0})
        }
        UiEvent::HoldEnded => json!({"type": "hold_ended"}),
        UiEvent::NearMiss { duration_ms, .. } => {
            json!({"type": "near_miss", "duration_ms": duration_ms})
        }
        UiEvent::JoinRequested {
            device,
            label,
            code,
        } => json!({
            "type": "join_requested",
            "device": device.to_hex(),
            "label": label,
            "code": code,
        }),
        UiEvent::JoinCode(c) => json!({"type": "join_code", "code": c}),
        UiEvent::JoinApproved { practice_name } => {
            json!({"type": "join_approved", "practice": practice_name})
        }
        UiEvent::Enrolled { practice_name } => {
            json!({"type": "enrolled", "practice": practice_name})
        }
        UiEvent::DrillReport(r) => json!({
            "type": "drill_report",
            "alarm_id": r.alarm_id.to_hex(),
            "rows": r.rows.len(),
        }),
        UiEvent::FewAdmins(n) => json!({"type": "few_admins", "admins": n}),
    }
}

fn role(r: muster_proto::Role) -> &'static str {
    match r {
        muster_proto::Role::Member => "member",
        muster_proto::Role::Admin => "admin",
        muster_proto::Role::Enroller => "enroller",
        muster_proto::Role::Other(_) => "unknown",
    }
}

/// Every device with a valid cert (members and admins), whether or not it
/// has been heard, with what is known of it.
pub fn devices(core: &Core, now: Mono, wall: u64) -> Value {
    let Some(t) = core.trust() else {
        return Value::Array(vec![]);
    };
    let me = core.device_id();
    let mut out = Vec::new();
    for d in t.members(wall) {
        let Some(best) = t
            .valid_certs(&d, wall)
            .into_iter()
            .max_by_key(|b| (b.role == muster_proto::Role::Admin, b.issued_at))
        else {
            continue;
        };
        let peer = core.peers().get(&d);
        let hb = peer.and_then(|p| p.heartbeat.as_ref());
        out.push(json!({
            "device": d.to_hex(),
            "device_short": d.short(),
            "this_device": d == me,
            "label": hb.map(|h| h.label.clone()).unwrap_or_else(|| best.label.clone()),
            "room": hb.map(|h| h.room.clone()),
            "user": hb.map(|h| h.user_display.clone()),
            "role": role(t.role(&d, wall).unwrap_or(best.role)),
            "enrolled_at": best.issued_at,
            "token_id": best.token_id.map(|x| x.to_hex()),
            "path": peer.map(|p| class(p.class(now))),
            "last_heard_ms": peer.and_then(|p| p.last_heard()).map(|t| now.since(t)),
            "status": peer.and_then(|p| p.status()).map(level),
        }));
    }
    Value::Array(out)
}

/// Enrolment tokens this practice knows of (their enroller certs), with the
/// devices each enrolled.
pub fn tokens(core: &Core, wall: u64) -> Value {
    let Some(t) = core.trust() else {
        return Value::Array(vec![]);
    };
    let certs: Vec<_> = t.certs().map(|(_, b)| b).collect();
    let mut out = Vec::new();
    for b in certs
        .iter()
        .filter(|b| b.role == muster_proto::Role::Enroller)
    {
        let Some(id) = b.token_id else { continue };
        let enrolled: Vec<Value> = certs
            .iter()
            .filter(|c| c.role != muster_proto::Role::Enroller && c.token_id == Some(id))
            .map(|c| json!({"device": c.subject.to_hex(), "label": c.label, "enrolled_at": c.issued_at}))
            .collect();
        out.push(json!({
            "token_id": id.to_hex(),
            "label": b.label,
            "issued_at": b.issued_at,
            "expires_at": b.expires_at,
            "expired": b.expires_at != 0 && wall > b.expires_at,
            "revoked": t.is_device_revoked(&b.subject),
            "enrolled": enrolled,
        }));
    }
    Value::Array(out)
}
