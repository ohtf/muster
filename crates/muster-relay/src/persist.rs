// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! `persist = true`: the ring buffer, learned certs and verified
//! revocations in SQLite (`<data_dir>/relay.sqlite`), so a restart neither
//! loses buffered envelopes nor forgets a revocation.
//!
//! Write errors are logged, not fatal: the relay keeps forwarding from
//! memory (availability first).

use std::path::Path;

use muster_crypto::RevocationRecord;
use muster_proto::{CertSerial, DeviceId, MsgId, PracticeId};
use rusqlite::{Connection, params};

#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

/// One buffered envelope as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEnvelope {
    pub cursor: u64,
    pub at_ms: u64,
    pub from: DeviceId,
    pub bytes: Vec<u8>,
}

/// Everything stored for one tenant.
#[derive(Debug, Default)]
pub struct TenantData {
    pub buffer: Vec<StoredEnvelope>,
    pub certs: Vec<Vec<u8>>,
    pub revocations: Vec<RevocationRecord>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS buffer (
    tenant BLOB NOT NULL, cursor INTEGER NOT NULL, at_ms INTEGER NOT NULL,
    sender BLOB NOT NULL, bytes BLOB NOT NULL, PRIMARY KEY (tenant, cursor));
CREATE TABLE IF NOT EXISTS certs (
    tenant BLOB NOT NULL, bytes BLOB NOT NULL, PRIMARY KEY (tenant, bytes));
CREATE TABLE IF NOT EXISTS revocations (
    tenant BLOB NOT NULL, msg_id BLOB NOT NULL, target_serial BLOB NOT NULL,
    target_device BLOB NOT NULL, cascade INTEGER NOT NULL, at INTEGER NOT NULL,
    PRIMARY KEY (tenant, msg_id));
";

fn arr<const N: usize>(v: Vec<u8>) -> rusqlite::Result<[u8; N]> {
    v.try_into().map_err(|v: Vec<u8>| {
        rusqlite::Error::FromSqlConversionFailure(
            v.len(),
            rusqlite::types::Type::Blob,
            "wrong length".into(),
        )
    })
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    fn logged(what: &str, r: rusqlite::Result<usize>) {
        if let Err(e) = r {
            tracing::warn!("relay store: {what}: {e}");
        }
    }

    pub fn put_envelope(&self, tenant: &PracticeId, e: &StoredEnvelope) {
        Self::logged(
            "buffer",
            self.conn.execute(
                "INSERT OR REPLACE INTO buffer VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    &tenant.0[..],
                    e.cursor as i64,
                    e.at_ms as i64,
                    &e.from.0[..],
                    e.bytes
                ],
            ),
        );
    }

    /// Drops buffered envelopes below `min_cursor`.
    pub fn prune(&self, tenant: &PracticeId, min_cursor: u64) {
        Self::logged(
            "prune",
            self.conn.execute(
                "DELETE FROM buffer WHERE tenant = ?1 AND cursor < ?2",
                params![&tenant.0[..], min_cursor as i64],
            ),
        );
    }

    pub fn put_cert(&self, tenant: &PracticeId, bytes: &[u8]) {
        Self::logged(
            "cert",
            self.conn.execute(
                "INSERT OR IGNORE INTO certs VALUES (?1, ?2)",
                params![&tenant.0[..], bytes],
            ),
        );
    }

    pub fn put_revocation(&self, tenant: &PracticeId, r: &RevocationRecord) {
        Self::logged(
            "revocation",
            self.conn.execute(
                "INSERT OR IGNORE INTO revocations VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &tenant.0[..],
                    &r.msg_id.0[..],
                    &r.target_serial.0[..],
                    &r.target_device.0[..],
                    r.cascade,
                    r.at as i64
                ],
            ),
        );
    }

    pub fn load(&self, tenant: &PracticeId) -> anyhow::Result<TenantData> {
        let t = &tenant.0[..];
        let mut s = self.conn.prepare(
            "SELECT cursor, at_ms, sender, bytes FROM buffer WHERE tenant = ?1 ORDER BY cursor",
        )?;
        let buffer = s
            .query_map([t], |r| {
                Ok(StoredEnvelope {
                    cursor: r.get::<_, i64>(0)? as u64,
                    at_ms: r.get::<_, i64>(1)? as u64,
                    from: DeviceId(arr(r.get(2)?)?),
                    bytes: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut s = self
            .conn
            .prepare("SELECT bytes FROM certs WHERE tenant = ?1")?;
        let certs = s
            .query_map([t], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut s = self.conn.prepare(
            "SELECT msg_id, target_serial, target_device, cascade, at FROM revocations \
             WHERE tenant = ?1 ORDER BY at",
        )?;
        let revocations = s
            .query_map([t], |r| {
                Ok(RevocationRecord {
                    msg_id: MsgId(arr(r.get(0)?)?),
                    target_serial: CertSerial(arr(r.get(1)?)?),
                    target_device: DeviceId(arr(r.get(2)?)?),
                    cascade: r.get(3)?,
                    at: r.get::<_, i64>(4)? as u64,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(TenantData {
            buffer,
            certs,
            revocations,
        })
    }
}
