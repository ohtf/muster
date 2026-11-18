// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Order-independent alarm state (`spec/03-protocol.md`, Alarm state;
//! FR-ALM-007).
//!
//! An alarm's state is a pure function of the set of valid messages about
//! it. Messages are stored keyed by `msg_id`; derivation never depends on
//! arrival order.

use std::collections::BTreeMap;

use crate::proto::{AlarmRaised, DeviceId, EventDetail, EventKind, MsgId, ReceiptDetail};

use crate::core::io::{AlarmPhase, AlarmView, LocalAck};
use crate::core::time::{ALARM_MAX_LIFE, FUTURE_SKEW_LIMIT, WallMs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaisedRec {
    pub created_at: WallMs,
    pub origin: DeviceId,
    pub payload: AlarmRaised,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRec {
    pub msg_id: MsgId,
    pub created_at: WallMs,
    pub sender: DeviceId,
    pub kind: EventKind,
    pub user_display: String,
    pub detail: EventDetail,
}

impl EventRec {
    fn key(&self) -> (WallMs, MsgId) {
        (self.created_at, self.msg_id)
    }
}

/// How an alarm closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closure {
    pub cancelled: bool,
    pub at: WallMs,
    pub msg_id: MsgId,
    pub by: DeviceId,
    pub by_name: String,
}

/// The set of messages known about one alarm.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlarmMessages {
    pub raised: Option<RaisedRec>,
    pub events: BTreeMap<MsgId, EventRec>,
}

/// State derived from [`AlarmMessages`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Derived {
    /// Latest receipt per device (origin excluded).
    pub receipts: BTreeMap<DeviceId, EventRec>,
    pub seen: BTreeMap<DeviceId, EventRec>,
    pub responding: BTreeMap<DeviceId, EventRec>,
    pub closed: Option<Closure>,
}

fn keep_latest(map: &mut BTreeMap<DeviceId, EventRec>, e: &EventRec) {
    match map.get(&e.sender) {
        Some(old) if old.key() >= e.key() => {}
        _ => {
            map.insert(e.sender, e.clone());
        }
    }
}

impl AlarmMessages {
    /// Derives the alarm state. Only meaningful once `raised` is known:
    /// before then, Cancelled events cannot be attributed to the origin.
    pub fn derive(&self) -> Derived {
        let mut d = Derived::default();
        let origin = self.raised.as_ref().map(|r| r.origin);
        for e in self.events.values() {
            match e.kind {
                EventKind::Receipt => {
                    if Some(e.sender) != origin {
                        keep_latest(&mut d.receipts, e);
                    }
                }
                EventKind::Seen => keep_latest(&mut d.seen, e),
                EventKind::Responding => keep_latest(&mut d.responding, e),
                EventKind::Resolved | EventKind::Cancelled => {
                    let cancelled = e.kind == EventKind::Cancelled;
                    // Only the origin may cancel (FR-ALM-005, P11).
                    if cancelled && Some(e.sender) != origin {
                        continue;
                    }
                    let better = d
                        .closed
                        .as_ref()
                        .is_none_or(|c| (e.created_at, e.msg_id) < (c.at, c.msg_id));
                    if better {
                        d.closed = Some(Closure {
                            cancelled,
                            at: e.created_at,
                            msg_id: e.msg_id,
                            by: e.sender,
                            by_name: e.user_display.clone(),
                        });
                    }
                }
                EventKind::Other(_) => {}
            }
        }
        d
    }

    /// Message ids of the raise and every event, for the alarm digest.
    pub fn message_ids(&self, alarm_id: MsgId) -> impl Iterator<Item = MsgId> + '_ {
        self.raised
            .as_ref()
            .map(|_| alarm_id)
            .into_iter()
            .chain(self.events.keys().copied())
    }
}

/// Age of an alarm at wall time `now`, applying the clock-skew rule: a
/// `raised_at` more than 5 minutes in the future counts as age 0. Never
/// drop an alarm because of skew. Returns (age, skewed).
pub fn alarm_age(raised_at: WallMs, now: WallMs) -> (u64, bool) {
    if raised_at > now.saturating_add(FUTURE_SKEW_LIMIT) {
        (0, true)
    } else {
        (now.saturating_sub(raised_at), false)
    }
}

/// Whether an open alarm is expired at `now`.
pub fn is_expired(raised_at: WallMs, now: WallMs) -> bool {
    alarm_age(raised_at, now).0 > ALARM_MAX_LIFE
}

/// Builds the UI view.
pub fn view(
    alarm_id: MsgId,
    msgs: &AlarmMessages,
    me: &DeviceId,
    local_ack: Option<LocalAck>,
    now: WallMs,
) -> Option<AlarmView> {
    let r = msgs.raised.as_ref()?;
    let d = msgs.derive();
    let state = match &d.closed {
        Some(c) if c.cancelled => AlarmPhase::Cancelled { at: c.at },
        Some(c) => AlarmPhase::Resolved {
            by: c.by_name.clone(),
            at: c.at,
        },
        None if is_expired(r.created_at, now) => AlarmPhase::Expired,
        None => AlarmPhase::Open,
    };
    let mut coming: Vec<&EventRec> = d.responding.values().collect();
    coming.sort_by_key(|e| e.key());
    let mut seen: Vec<&EventRec> = d
        .seen
        .values()
        .filter(|e| !d.responding.contains_key(&e.sender))
        .collect();
    seen.sort_by_key(|e| e.key());
    Some(AlarmView {
        alarm_id,
        origin: r.origin,
        own: &r.origin == me,
        drill: r.payload.drill,
        room: r.payload.room.clone(),
        user_display: r.payload.user_display.clone(),
        device_label: r.payload.device_label.clone(),
        raised_at: r.created_at,
        source: r.payload.source,
        state,
        delivered: d.receipts.len(),
        coming: coming.iter().map(|e| e.user_display.clone()).collect(),
        seen: seen.iter().map(|e| e.user_display.clone()).collect(),
        local_ack,
    })
}

/// The receipt detail of a device, if it sent one.
pub fn receipt_of<'a>(d: &'a Derived, dev: &DeviceId) -> Option<&'a ReceiptDetail> {
    match &d.receipts.get(dev)?.detail {
        EventDetail::Receipt(r) => Some(r),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{AlarmKind, Source};
    use proptest::prelude::*;

    fn dev(n: u8) -> DeviceId {
        DeviceId([n; 32])
    }

    fn raised(origin: u8) -> RaisedRec {
        RaisedRec {
            created_at: 1000,
            origin: dev(origin),
            payload: AlarmRaised {
                alarm_id: MsgId([9; 16]),
                kind: AlarmKind::Help,
                drill: false,
                room: "Room 4".into(),
                user_display: "Dr A".into(),
                device_label: "PC4".into(),
                source: Source::Button,
            },
        }
    }

    fn ev(id: u8, at: u64, sender: u8, kind: EventKind) -> EventRec {
        EventRec {
            msg_id: MsgId([id; 16]),
            created_at: at,
            sender: dev(sender),
            kind,
            user_display: format!("user{sender}"),
            detail: EventDetail::None,
        }
    }

    fn msgs(events: &[EventRec]) -> AlarmMessages {
        AlarmMessages {
            raised: Some(raised(1)),
            events: events.iter().map(|e| (e.msg_id, e.clone())).collect(),
        }
    }

    #[test]
    fn p11_only_origin_cancels() {
        let m = msgs(&[ev(1, 2000, 2, EventKind::Cancelled)]);
        assert_eq!(m.derive().closed, None);
        let m = msgs(&[ev(1, 2000, 1, EventKind::Cancelled)]);
        assert!(m.derive().closed.is_some_and(|c| c.cancelled));
    }

    #[test]
    fn earliest_closure_wins() {
        let m = msgs(&[
            ev(1, 3000, 2, EventKind::Resolved),
            ev(2, 2500, 1, EventKind::Cancelled),
            ev(3, 2500, 3, EventKind::Resolved),
        ]);
        let c = m.derive().closed.expect("closed");
        // Same time: lower msg_id wins.
        assert_eq!(c.msg_id, MsgId([2; 16]));
        assert!(c.cancelled);
    }

    #[test]
    fn p10_delivered_counts_distinct_receipt_senders_not_origin() {
        let m = msgs(&[
            ev(1, 1100, 2, EventKind::Receipt),
            ev(2, 1200, 2, EventKind::Receipt),
            ev(3, 1100, 3, EventKind::Receipt),
            ev(4, 1100, 1, EventKind::Receipt),
        ]);
        let v = view(MsgId([9; 16]), &m, &dev(1), None, 1500).expect("view");
        assert_eq!(v.delivered, 2);
    }

    #[test]
    fn coming_and_seen_lists() {
        let m = msgs(&[
            ev(1, 1300, 3, EventKind::Responding),
            ev(2, 1200, 2, EventKind::Responding),
            ev(3, 1100, 4, EventKind::Seen),
            ev(4, 1100, 2, EventKind::Seen),
        ]);
        let v = view(MsgId([9; 16]), &m, &dev(5), None, 1500).expect("view");
        assert_eq!(v.coming, vec!["user2", "user3"]);
        assert_eq!(v.seen, vec!["user4"]);
    }

    #[test]
    fn fr_alm_008_expiry_and_skew() {
        assert!(!is_expired(0, ALARM_MAX_LIFE));
        assert!(is_expired(0, ALARM_MAX_LIFE + 1));
        // Far-future raised_at counts as age 0 (never dropped for skew).
        assert_eq!(alarm_age(10 * 60_000, 0), (0, true));
        assert_eq!(alarm_age(4 * 60_000, 0), (0, false));
    }

    proptest! {
        /// FR-ALM-007 / P4: every permutation of the same events derives the
        /// same state.
        #[test]
        fn derivation_is_order_independent(
            specs in proptest::collection::vec((0u8..40, 0u64..5000, 1u8..6, 0u64..5), 0..20),
            seed in any::<u64>(),
        ) {
            let events: Vec<EventRec> = specs
                .iter()
                .map(|(id, at, s, k)| ev(*id, *at, *s, EventKind::from_code(*k)))
                .collect();
            let a = msgs(&events).derive();
            let mut shuffled = events.clone();
            // Deterministic Fisher-Yates.
            let mut x = seed | 1;
            for i in (1..shuffled.len()).rev() {
                x ^= x << 13; x ^= x >> 7; x ^= x << 17;
                shuffled.swap(i, (x % (i as u64 + 1)) as usize);
            }
            let mut m = AlarmMessages { raised: Some(raised(1)), events: BTreeMap::new() };
            for e in shuffled {
                m.events.entry(e.msg_id).or_insert(e);
            }
            // Duplicate msg_ids with different content: first insertion wins
            // in both orders only if ids are unique, so compare on unique ids.
            let unique: BTreeMap<MsgId, EventRec> = events.iter().map(|e| (e.msg_id, e.clone())).collect();
            if unique.len() == events.len() {
                prop_assert_eq!(a, m.derive());
            }
        }
    }
}
