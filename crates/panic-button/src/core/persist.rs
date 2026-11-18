// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Persistence operations and the state they build.
//!
//! The core emits [`PersistOp`]s; the runtime applies them in order (fsync
//! for security-relevant ones) and passes the resulting [`PersistedState`]
//! back to [`crate::core::Core::new`] on start. [`PersistedState::apply`] is the
//! reference implementation the store and simulator both use, so a restart
//! sees exactly what was persisted (P3).

use std::collections::BTreeMap;

use crate::crypto::GroupKey;
use crate::proto::{CertSerial, MsgId, PracticeId};

use crate::core::io::LocalAck;
use crate::core::time::WallMs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PracticeInfo {
    pub practice_id: PracticeId,
    pub root_pub: [u8; 32],
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGroupKey {
    pub epoch: u64,
    pub key: GroupKey,
    /// When this device first held the key.
    pub at: WallMs,
}

/// Local, per-device alarm state (not shared on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlarmLocal {
    pub alerted: bool,
    pub local_ack: Option<LocalAck>,
    /// Went to history without alerting.
    pub history_only: bool,
    pub first_seen: WallMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoredAlarm {
    /// Raw envelopes (raise and events), for restore and for sync.
    pub envelopes: BTreeMap<MsgId, Vec<u8>>,
    pub local: AlarmLocal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistOp {
    SetPractice(PracticeInfo),
    SetOwnSerial(CertSerial),
    AddCert(Vec<u8>),
    AddRevocation(Vec<u8>),
    AddGroupKey(StoredGroupKey),
    AddKeyShare(Vec<u8>),
    SetConfig(Vec<u8>),
    /// This device's own label and room (a DeviceSettings, ADR 0014).
    SetOwnSettings(Vec<u8>),
    AddAlarmEnvelope {
        alarm_id: MsgId,
        msg_id: MsgId,
        bytes: Vec<u8>,
    },
    SetAlarmLocal {
        alarm_id: MsgId,
        local: AlarmLocal,
    },
    /// A processed alarm-related message id (FR-ALM-010).
    Processed {
        msg_id: MsgId,
        at: WallMs,
    },
    /// Forget an alarm's envelopes and processed ids older than retention.
    PruneAlarm(MsgId),
    PruneProcessedBefore(WallMs),
}

impl PersistOp {
    /// Security-relevant ops are fsynced before dependent outputs are
    /// released (`spec/02-architecture.md`, Runtime).
    pub fn must_sync(&self) -> bool {
        matches!(
            self,
            PersistOp::SetPractice(_)
                | PersistOp::SetOwnSerial(_)
                | PersistOp::AddCert(_)
                | PersistOp::AddRevocation(_)
                | PersistOp::AddGroupKey(_)
                | PersistOp::Processed { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistedState {
    pub practice: Option<PracticeInfo>,
    pub own_serial: Option<CertSerial>,
    pub certs: Vec<Vec<u8>>,
    pub revocations: Vec<Vec<u8>>,
    pub group_keys: Vec<StoredGroupKey>,
    pub keyshares: Vec<Vec<u8>>,
    pub config: Option<Vec<u8>>,
    pub own_settings: Option<Vec<u8>>,
    pub alarms: BTreeMap<MsgId, StoredAlarm>,
    pub processed: BTreeMap<MsgId, WallMs>,
}

impl PersistedState {
    pub fn apply(&mut self, op: PersistOp) {
        match op {
            PersistOp::SetPractice(p) => self.practice = Some(p),
            PersistOp::SetOwnSerial(s) => self.own_serial = Some(s),
            PersistOp::AddCert(c) => {
                if !self.certs.contains(&c) {
                    self.certs.push(c);
                }
            }
            PersistOp::AddRevocation(r) => {
                if !self.revocations.contains(&r) {
                    self.revocations.push(r);
                }
            }
            PersistOp::AddGroupKey(k) => {
                if !self
                    .group_keys
                    .iter()
                    .any(|g| g.epoch == k.epoch && g.key == k.key)
                {
                    self.group_keys.push(k);
                }
            }
            PersistOp::AddKeyShare(k) => {
                if !self.keyshares.contains(&k) {
                    self.keyshares.push(k);
                }
            }
            PersistOp::SetConfig(c) => self.config = Some(c),
            PersistOp::SetOwnSettings(s) => self.own_settings = Some(s),
            PersistOp::AddAlarmEnvelope {
                alarm_id,
                msg_id,
                bytes,
            } => {
                self.alarms
                    .entry(alarm_id)
                    .or_default()
                    .envelopes
                    .entry(msg_id)
                    .or_insert(bytes);
            }
            PersistOp::SetAlarmLocal { alarm_id, local } => {
                self.alarms.entry(alarm_id).or_default().local = local;
            }
            PersistOp::Processed { msg_id, at } => {
                self.processed.entry(msg_id).or_insert(at);
            }
            PersistOp::PruneAlarm(id) => {
                self.alarms.remove(&id);
            }
            PersistOp::PruneProcessedBefore(t) => self.processed.retain(|_, at| *at >= t),
        }
    }
}

impl PersistedState {
    /// Operations that rebuild this state from empty (used to save the
    /// result of a bootstrap flow through the same path as live updates).
    pub fn to_ops(&self) -> Vec<PersistOp> {
        let mut v = Vec::new();
        if let Some(p) = &self.practice {
            v.push(PersistOp::SetPractice(p.clone()));
        }
        if let Some(s) = self.own_serial {
            v.push(PersistOp::SetOwnSerial(s));
        }
        v.extend(self.certs.iter().cloned().map(PersistOp::AddCert));
        v.extend(
            self.revocations
                .iter()
                .cloned()
                .map(PersistOp::AddRevocation),
        );
        v.extend(self.group_keys.iter().cloned().map(PersistOp::AddGroupKey));
        v.extend(self.keyshares.iter().cloned().map(PersistOp::AddKeyShare));
        if let Some(c) = &self.config {
            v.push(PersistOp::SetConfig(c.clone()));
        }
        if let Some(s) = &self.own_settings {
            v.push(PersistOp::SetOwnSettings(s.clone()));
        }
        for (id, a) in &self.alarms {
            for (m, b) in &a.envelopes {
                v.push(PersistOp::AddAlarmEnvelope {
                    alarm_id: *id,
                    msg_id: *m,
                    bytes: b.clone(),
                });
            }
            v.push(PersistOp::SetAlarmLocal {
                alarm_id: *id,
                local: a.local,
            });
        }
        for (m, at) in &self.processed {
            v.push(PersistOp::Processed {
                msg_id: *m,
                at: *at,
            });
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ops_rebuilds_the_state() {
        let mut s = PersistedState::default();
        s.apply(PersistOp::AddCert(vec![1]));
        s.apply(PersistOp::Processed {
            msg_id: MsgId([1; 16]),
            at: 5,
        });
        s.apply(PersistOp::SetAlarmLocal {
            alarm_id: MsgId([2; 16]),
            local: AlarmLocal::default(),
        });
        let mut t = PersistedState::default();
        for op in s.to_ops() {
            t.apply(op);
        }
        assert_eq!(s, t);
    }
}
