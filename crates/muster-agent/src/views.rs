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

fn cue_name(c: muster_proto::CueMode) -> &'static str {
    match c {
        muster_proto::CueMode::Silent => "silent",
        muster_proto::CueMode::Soft => "soft",
        muster_proto::CueMode::AlarmRamp => "alarm_ramp",
        muster_proto::CueMode::Other(_) => "unknown",
    }
}

fn pattern_name(p: muster_proto::SoundPattern) -> &'static str {
    match p {
        muster_proto::SoundPattern::A => "A",
        muster_proto::SoundPattern::B => "B",
        muster_proto::SoundPattern::C => "C",
        muster_proto::SoundPattern::Other(_) => "unknown",
    }
}

fn drill_name(d: muster_proto::DrillMode) -> &'static str {
    match d {
        muster_proto::DrillMode::Silent => "silent",
        muster_proto::DrillMode::Audible => "audible",
        muster_proto::DrillMode::Other(_) => "unknown",
    }
}

/// Practice-wide settings (FR-ADM-003); unset fields are null.
pub fn practice_settings(s: &muster_proto::PracticeSettings, version: u64) -> Value {
    json!({
        "version": version,
        "hold_ms": s.hold_ms,
        "cue_mode": s.cue_mode.map(cue_name),
        "sound_pattern": s.sound_pattern.map(pattern_name),
        "override_mute": s.override_mute,
        "drill_mode": s.drill_mode.map(drill_name),
        "drill_schedule": s.drill_schedule,
        "drill_coordinator": s.drill_coordinator.map(|d| d.to_hex()),
        "opening_hours": s.opening_hours,
        "relay_urls": s.relay_urls,
        "seed_peers": s.seed_peers,
    })
}

/// Applies the fields present in `p` to `cur`. A field given as null
/// clears it (back to the default); an absent field is unchanged.
pub fn merge_practice_settings(
    mut cur: muster_proto::PracticeSettings,
    p: &Value,
) -> Result<muster_proto::PracticeSettings, String> {
    use muster_proto::{CueMode, DrillMode, SoundPattern};
    let Some(m) = p.as_object() else {
        return Err("settings must be an object".into());
    };
    for (k, v) in m {
        let s = v.as_str();
        match k.as_str() {
            "hold_ms" => {
                cur.hold_ms = match v.as_u64() {
                    None if v.is_null() => None,
                    Some(h) if (1000..=3000).contains(&h) => Some(h as u32),
                    _ => return Err("hold_ms must be 1000 to 3000".into()),
                }
            }
            "cue_mode" => {
                cur.cue_mode = match s {
                    None if v.is_null() => None,
                    Some("silent") => Some(CueMode::Silent),
                    Some("soft") => Some(CueMode::Soft),
                    Some("alarm_ramp") => Some(CueMode::AlarmRamp),
                    _ => return Err("cue_mode must be silent, soft or alarm_ramp".into()),
                }
            }
            "sound_pattern" => {
                cur.sound_pattern = match s {
                    None if v.is_null() => None,
                    Some("A") => Some(SoundPattern::A),
                    Some("B") => Some(SoundPattern::B),
                    Some("C") => Some(SoundPattern::C),
                    _ => return Err("sound_pattern must be A, B or C".into()),
                }
            }
            "drill_mode" => {
                cur.drill_mode = match s {
                    None if v.is_null() => None,
                    Some("silent") => Some(DrillMode::Silent),
                    Some("audible") => Some(DrillMode::Audible),
                    _ => return Err("drill_mode must be silent or audible".into()),
                }
            }
            "override_mute" => {
                cur.override_mute = match v {
                    Value::Null => None,
                    Value::Bool(b) => Some(*b),
                    _ => return Err("override_mute must be true or false".into()),
                }
            }
            "opening_hours" | "drill_schedule" => {
                let t = match v {
                    Value::Null => None,
                    Value::String(t) if t.len() <= muster_proto::limits::CONFIG_STRING_MAX => {
                        Some(t.trim().to_owned()).filter(|t| !t.is_empty())
                    }
                    _ => return Err(format!("{k} must be text")),
                };
                if k == "opening_hours" {
                    cur.opening_hours = t;
                } else {
                    cur.drill_schedule = t;
                }
            }
            "relay_urls" | "seed_peers" => {
                let list = match v {
                    Value::Null => None,
                    Value::Array(a) => {
                        let mut out = Vec::new();
                        for x in a {
                            let x = x
                                .as_str()
                                .ok_or(format!("{k} must be a list of text"))?
                                .trim();
                            if k == "relay_urls"
                                && !(x.starts_with("wss://") || x.starts_with("ws://"))
                            {
                                return Err(format!("relay URL {x:?} must start with wss://"));
                            }
                            if !x.is_empty() {
                                out.push(x.to_owned());
                            }
                        }
                        if out.len() > 64 {
                            return Err(format!("{k}: at most 64"));
                        }
                        Some(out).filter(|o| !o.is_empty())
                    }
                    _ => return Err(format!("{k} must be a list")),
                };
                if k == "relay_urls" {
                    cur.relay_urls = list;
                } else {
                    cur.seed_peers = list;
                }
            }
            "drill_coordinator" => {
                cur.drill_coordinator = match s {
                    None if v.is_null() => None,
                    Some(h) => Some(
                        muster_proto::DeviceId::from_hex(h)
                            .ok_or("drill_coordinator must be a device id")?,
                    ),
                    None => return Err("drill_coordinator must be a device id".into()),
                }
            }
            other => return Err(format!("unknown setting {other}")),
        }
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_merge_and_validate() {
        let s = merge_practice_settings(
            Default::default(),
            &json!({"hold_ms": 2000, "cue_mode": "silent", "relay_urls": ["wss://r.example/v1/ws"]}),
        )
        .expect("valid");
        assert_eq!(s.hold_ms, Some(2000));
        assert_eq!(
            s.relay_urls.as_deref(),
            Some(&["wss://r.example/v1/ws".to_owned()][..])
        );
        let s = merge_practice_settings(s, &json!({"hold_ms": null})).expect("clear");
        assert_eq!(s.hold_ms, None);
        assert_eq!(s.cue_mode, Some(muster_proto::CueMode::Silent), "untouched");
        assert!(merge_practice_settings(s.clone(), &json!({"hold_ms": 50})).is_err());
        assert!(merge_practice_settings(s.clone(), &json!({"relay_urls": ["http://x"]})).is_err());
        assert!(merge_practice_settings(s, &json!({"colour": "red"})).is_err());
        let v = practice_settings(&Default::default(), 0);
        assert!(v["hold_ms"].is_null());
    }
}
