// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Persistence operations and the state they build.
//!
//! The core emits [`PersistOp`]s; the runtime applies them in order (fsync
//! for security-relevant ones) and passes the resulting [`PersistedState`]
//! back to [`crate::Core::new`] on start. [`PersistedState::apply`] is the
//! reference implementation the store and simulator both use, so a restart
//! sees exactly what was persisted (P3).

use std::collections::BTreeMap;

use muster_crypto::GroupKey;
use muster_proto::{CertSerial, MsgId, PracticeId};

use crate::io::LocalAck;
use crate::time::WallMs;

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
