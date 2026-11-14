// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

use muster_core::io::{AuditEvent, LocalAck};
use muster_core::persist::{AlarmLocal, PersistOp, PersistedState, PracticeInfo, StoredGroupKey};
use muster_crypto::{DeviceKeys, GroupKey};
use muster_proto::{CertSerial, MsgId, PracticeId, Source};

use super::Store;

fn ops() -> Vec<PersistOp> {
    let a = MsgId([1; 16]);
    let b = MsgId([2; 16]);
    vec![
        PersistOp::SetPractice(PracticeInfo {
            practice_id: PracticeId([3; 16]),
            root_pub: [4; 32],
            name: "Riverside Surgery".into(),
        }),
        PersistOp::SetOwnSerial(CertSerial([5; 16])),
        PersistOp::AddCert(vec![1, 2, 3]),
        PersistOp::AddCert(vec![0]),
        PersistOp::AddCert(vec![1, 2, 3]),
        PersistOp::AddRevocation(vec![9]),
        PersistOp::AddGroupKey(StoredGroupKey {
            epoch: 0,
            key: GroupKey::from_bytes([6; 32]),
            at: 10,
        }),
        PersistOp::AddGroupKey(StoredGroupKey {
            epoch: 1,
            key: GroupKey::from_bytes([7; 32]),
            at: 20,
        }),
        PersistOp::AddGroupKey(StoredGroupKey {
            epoch: 1,
            key: GroupKey::from_bytes([7; 32]),
            at: 30,
        }),
        PersistOp::AddKeyShare(vec![8, 8]),
        PersistOp::SetConfig(vec![1]),
        PersistOp::SetConfig(vec![2]),
        PersistOp::AddAlarmEnvelope {
            alarm_id: a,
            msg_id: a,
            bytes: vec![1],
        },
        PersistOp::AddAlarmEnvelope {
            alarm_id: a,
            msg_id: b,
            bytes: vec![2],
        },
        PersistOp::AddAlarmEnvelope {
            alarm_id: a,
            msg_id: b,
            bytes: vec![99],
        },
        PersistOp::SetAlarmLocal {
            alarm_id: a,
            local: AlarmLocal {
                alerted: true,
                local_ack: Some(LocalAck::Responding),
                history_only: false,
                first_seen: 5,
            },
        },
        PersistOp::SetAlarmLocal {
            alarm_id: b,
            local: AlarmLocal::default(),
        },
        PersistOp::Processed { msg_id: a, at: 100 },
        PersistOp::Processed { msg_id: a, at: 200 },
        PersistOp::Processed { msg_id: b, at: 50 },
        PersistOp::PruneProcessedBefore(60),
        PersistOp::PruneAlarm(b),
    ]
}

#[test]
fn store_matches_persisted_state_apply() {
    let mut expected = PersistedState::default();
    let store = Store::open_in_memory().expect("open");
    for op in ops() {
        expected.apply(op.clone());
        store.apply(&op).expect("apply");
    }
    assert_eq!(store.load().expect("load"), expected);
}

#[test]
fn batches_survive_reopen() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("muster.db");
    let mut expected = PersistedState::default();
    {
        let mut store = Store::open(&path).expect("open");
        let ops = ops();
        store.apply_all(ops.iter()).expect("batch");
        for op in ops {
            expected.apply(op);
        }
        let keys = DeviceKeys::from_secrets([1; 32], [2; 32]);
        store.save_identity(&keys).expect("identity");
        store
            .audit(
                7,
                &AuditEvent::NearMiss {
                    source: Source::Button,
                    duration_ms: 800,
                },
            )
            .expect("audit");
    }
    let store = Store::open(&path).expect("reopen");
    assert_eq!(store.load().expect("load"), expected);
    let keys = store.load_identity().expect("identity").expect("present");
    assert_eq!(
        keys.device_id(),
        DeviceKeys::from_secrets([1; 32], [2; 32]).device_id()
    );
    let audit = store.audit_entries().expect("audit");
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].1, "NearMiss");
    assert_eq!(store.prune_audit(8).expect("prune"), 1);
}

#[test]
fn refuses_newer_schema() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("muster.db");
    {
        let c = rusqlite::Connection::open(&path).expect("open");
        c.pragma_update(None, "user_version", 99).expect("pragma");
    }
    assert!(matches!(
        Store::open(&path),
        Err(super::StoreError::TooNew(99))
    ));
}
