// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Relay state: tenants, sessions, the per-tenant ring buffer, presence and
//! revocation enforcement (`spec/06-relay.md`). No sockets here: the server
//! feeds it frames and delivers what it returns, so it is unit-tested
//! directly.
//!
//! The relay never holds group keys. It parses sealed envelopes only as far
//! as the header, and verifies open envelopes (RelayAuth, Revocation)
//! against the tenant's root key.

use std::collections::{HashMap, HashSet, VecDeque};

use muster_crypto::{RevocationRecord, TrustStore, verify_signed};
use muster_proto::relay::{AuthResult, Challenge, Frame, Presence};
use muster_proto::{
    Cbor, Cert, DeviceId, Envelope, MsgType, Payload, PracticeId, PracticeTag, SignedMessage,
};
use tokio::sync::mpsc;

use crate::config::Limits;
use crate::persist::{Store, StoredEnvelope};

pub use muster_proto::relay::{
    CLOSE_AUTH_FAILED, CLOSE_RATE_LIMITED, CLOSE_REVOKED, CLOSE_UNKNOWN_TENANT,
};

pub type SessionId = [u8; 16];

/// A long-poll session that has not polled or sent for this long is gone
/// (clients poll with a 25 s timeout).
pub const LP_IDLE_MS: u64 = 60_000;
/// Revocations whose signer the relay does not know yet, kept per tenant
/// until an auth chain introduces the signer.
const MAX_PENDING_REVOCATIONS: usize = 64;

/// The first cursor of a tenant in this relay process: derived from the
/// clock, so cursors keep increasing across restarts even without
/// `persist` and a client resuming from an older cursor misses nothing.
fn first_cursor(now_ms: u64) -> u64 {
    now_ms.saturating_mul(1000).max(1)
}

/// Something to deliver to a WebSocket session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outgoing {
    Frame(Frame),
    Close(u16),
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_ms: u64,
}

#[derive(Debug)]
struct Session {
    tenant: PracticeId,
    device: DeviceId,
    /// None for long-poll sessions (they read the buffer).
    tx: Option<mpsc::UnboundedSender<Outgoing>>,
    bucket: Bucket,
    expires_ms: Option<u64>,
    last_active_ms: u64,
}

#[derive(Debug)]
struct Buffered {
    cursor: u64,
    at_ms: u64,
    from: DeviceId,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct Tenant {
    name: String,
    trust: TrustStore,
    tags: HashSet<PracticeTag>,
    buffer: VecDeque<Buffered>,
    next_cursor: u64,
    notify: std::sync::Arc<tokio::sync::Notify>,
    pending_revocations: Vec<Vec<u8>>,
}

/// Counters for `/metrics` (counts only, never content).
#[derive(Debug, Default, Clone)]
pub struct Metrics {
    pub auth_ok: u64,
    pub auth_failed: u64,
    pub envelopes_in: u64,
    pub envelopes_forwarded: u64,
    pub dropped_rate_limited: u64,
    pub dropped_invalid: u64,
    pub revocations: u64,
}

#[derive(Debug)]
pub struct Relay {
    limits: Limits,
    tenants: HashMap<PracticeId, Tenant>,
    sessions: HashMap<SessionId, Session>,
    store: Option<Store>,
    pub metrics: Metrics,
}

/// Why authentication failed, with its close code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthError(pub u16);

impl Relay {
    pub fn new(limits: Limits) -> Self {
        Relay {
            limits,
            tenants: HashMap::new(),
            sessions: HashMap::new(),
            store: None,
            metrics: Metrics::default(),
        }
    }

    pub fn add_tenant(
        &mut self,
        practice_id: PracticeId,
        root_pub: [u8; 32],
        name: &str,
        now_ms: u64,
    ) {
        self.tenants.entry(practice_id).or_insert_with(|| Tenant {
            name: name.to_owned(),
            trust: TrustStore::new(practice_id, root_pub),
            tags: HashSet::new(),
            buffer: VecDeque::new(),
            next_cursor: first_cursor(now_ms),
            notify: Default::default(),
            pending_revocations: Vec::new(),
        });
    }

    /// Persists from now on, after loading what the store holds for the
    /// tenants already added: certs, then revocations, then the buffer.
    pub fn attach_store(&mut self, store: Store, now_ms: u64) -> anyhow::Result<()> {
        let ids: Vec<PracticeId> = self.tenants.keys().copied().collect();
        for id in ids {
            let data = store.load(&id)?;
            let Some(t) = self.tenants.get_mut(&id) else {
                continue;
            };
            for raw in &data.certs {
                if let Ok(c) = Cert::from_cbor(raw) {
                    let _ = t.trust.add_cert(c);
                }
            }
            for r in data.revocations {
                t.trust.add_revocation(r);
            }
            for e in data.buffer {
                t.next_cursor = t.next_cursor.max(e.cursor + 1);
                t.buffer.push_back(Buffered {
                    cursor: e.cursor,
                    at_ms: e.at_ms,
                    from: e.from,
                    bytes: e.bytes,
                });
            }
        }
        self.store = Some(store);
        let ids: Vec<PracticeId> = self.tenants.keys().copied().collect();
        for id in ids {
            self.prune_buffer(id, now_ms);
        }
        Ok(())
    }

    pub fn tenant_count(&self) -> usize {
        self.tenants.len()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn challenge(nonce: [u8; 32], now_ms: u64) -> Challenge {
        Challenge {
            nonce,
            server_time: now_ms,
            relay_version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    /// Checks a RelayAuth envelope against the challenge nonce and the
    /// tenant root. On success registers the session and returns the auth
    /// result plus the envelopes to replay (after `resume_cursor`, else the
    /// last `buffer_hours`).
    pub fn authenticate(
        &mut self,
        session: SessionId,
        nonce: &[u8; 32],
        envelope: &[u8],
        tx: Option<mpsc::UnboundedSender<Outgoing>>,
        now_ms: u64,
    ) -> Result<(AuthResult, Vec<Vec<u8>>), AuthError> {
        let r = self.check_auth(nonce, envelope, now_ms);
        let (practice_id, device, resume) = match r {
            Ok(x) => x,
            Err(e) => {
                self.metrics.auth_failed += 1;
                return Err(e);
            }
        };
        // The chain may have made a pending revocation verifiable.
        self.enforce_revocations(practice_id, now_ms);
        let tenant_sessions = self
            .sessions
            .values()
            .filter(|s| s.tenant == practice_id)
            .count();
        if tenant_sessions >= self.limits.max_connections_per_tenant {
            return Err(AuthError(CLOSE_RATE_LIMITED));
        }
        self.metrics.auth_ok += 1;
        let long_poll = tx.is_none();
        self.sessions.insert(
            session,
            Session {
                tenant: practice_id,
                device,
                tx,
                bucket: Bucket {
                    tokens: f64::from(self.limits.burst_per_conn),
                    last_ms: now_ms,
                },
                expires_ms: long_poll.then_some(now_ms + 60 * 60 * 1000),
                last_active_ms: now_ms,
            },
        );
        self.prune_buffer(practice_id, now_ms);
        let t = self
            .tenants
            .get(&practice_id)
            .ok_or(AuthError(CLOSE_UNKNOWN_TENANT))?;
        let replay: Vec<Vec<u8>> = t
            .buffer
            .iter()
            .filter(|b| resume.is_none_or(|c| b.cursor > c) && b.from != device)
            .map(|b| b.bytes.clone())
            .collect();
        let cursor = t.next_cursor - 1;
        let presence = self.presence_of(practice_id, Some(session));
        self.broadcast_presence(practice_id);
        Ok((
            AuthResult {
                ok: true,
                session_id: session,
                cursor,
                presence,
            },
            replay,
        ))
    }

    fn check_auth(
        &mut self,
        nonce: &[u8; 32],
        envelope: &[u8],
        now_ms: u64,
    ) -> Result<(PracticeId, DeviceId, Option<u64>), AuthError> {
        let fail = AuthError(CLOSE_AUTH_FAILED);
        let Ok(Envelope::Open(o)) = Envelope::from_cbor(envelope) else {
            return Err(fail);
        };
        let signed = SignedMessage::from_cbor(&o.payload).map_err(|_| fail)?;
        let body = verify_signed(&signed, true).map_err(|_| fail)?;
        let Payload::RelayAuth(auth) = body.payload else {
            return Err(fail);
        };
        if auth.practice_id != o.practice_id || &auth.nonce != nonce {
            return Err(fail);
        }
        let t = self
            .tenants
            .get_mut(&auth.practice_id)
            .ok_or(AuthError(CLOSE_UNKNOWN_TENANT))?;
        let mut learned = false;
        for raw in &auth.chain {
            if let Ok(c) = Cert::from_cbor(raw)
                && let Ok((_, true)) = t.trust.add_cert(c)
            {
                learned = true;
                if let Some(st) = &self.store {
                    st.put_cert(&auth.practice_id, raw);
                }
            }
        }
        if learned && !t.pending_revocations.is_empty() {
            for raw in std::mem::take(&mut t.pending_revocations) {
                match apply_revocation(&auth.practice_id, t, self.store.as_ref(), &raw, now_ms) {
                    Some(true) => self.metrics.revocations += 1,
                    Some(false) => {}
                    None => t.pending_revocations.push(raw),
                }
            }
        }
        if t.trust.is_device_revoked(&body.sender) {
            return Err(AuthError(CLOSE_REVOKED));
        }
        if !t.trust.is_member(&body.sender, now_ms) {
            return Err(fail);
        }
        Ok((auth.practice_id, body.sender, auth.resume_cursor))
    }

    fn presence_of(&self, tenant: PracticeId, except: Option<SessionId>) -> Vec<DeviceId> {
        let mut v: Vec<DeviceId> = self
            .sessions
            .iter()
            .filter(|(id, s)| s.tenant == tenant && Some(**id) != except)
            .map(|(_, s)| s.device)
            .collect();
        v.sort();
        v.dedup();
        v
    }

    fn broadcast_presence(&self, tenant: PracticeId) {
        // Wake long-poll waiters so they see the new presence.
        if let Some(t) = self.tenants.get(&tenant) {
            t.notify.notify_waiters();
        }
        for (id, s) in &self.sessions {
            if s.tenant != tenant {
                continue;
            }
            if let Some(tx) = &s.tx {
                let connected = self.presence_of(tenant, Some(*id));
                let _ = tx.send(Outgoing::Frame(Frame::Presence(Presence { connected })));
            }
        }
    }

    pub fn disconnect(&mut self, session: &SessionId) {
        if let Some(s) = self.sessions.remove(session) {
            self.broadcast_presence(s.tenant);
        }
    }

    /// Ends long-poll sessions that expired or went idle (`LP_IDLE_MS`),
    /// so presence does not show devices that have gone.
    pub fn expire(&mut self, now_ms: u64) {
        let gone: Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|(_, s)| {
                s.tx.is_none()
                    && (s.expires_ms.is_some_and(|e| now_ms > e)
                        || now_ms.saturating_sub(s.last_active_ms) > LP_IDLE_MS)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            self.disconnect(&id);
        }
    }

    fn rate_ok(&mut self, session: &SessionId, now_ms: u64) -> bool {
        let rate = f64::from(self.limits.msgs_per_sec_per_conn);
        let burst = f64::from(self.limits.burst_per_conn);
        let Some(s) = self.sessions.get_mut(session) else {
            return false;
        };
        let b = &mut s.bucket;
        b.tokens = (b.tokens + rate * now_ms.saturating_sub(b.last_ms) as f64 / 1000.0).min(burst);
        b.last_ms = now_ms;
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// An envelope from an authenticated session: validated as far as the
    /// relay can, buffered and forwarded. Returns false if the session
    /// should be closed.
    pub fn receive(&mut self, session: &SessionId, bytes: &[u8], now_ms: u64) -> bool {
        let Some(s) = self.sessions.get_mut(session) else {
            return false;
        };
        s.last_active_ms = now_ms;
        let (tenant_id, from) = (s.tenant, s.device);
        if bytes.len() > self.limits.max_frame_bytes {
            self.metrics.dropped_invalid += 1;
            return true;
        }
        if !self.rate_ok(session, now_ms) {
            self.metrics.dropped_rate_limited += 1;
            return true;
        }
        self.metrics.envelopes_in += 1;
        let Ok(env) = Envelope::from_cbor(bytes) else {
            self.metrics.dropped_invalid += 1;
            return true;
        };
        let Some(t) = self.tenants.get_mut(&tenant_id) else {
            return false;
        };
        match &env {
            // Learn tags from authenticated clients of this tenant only.
            Envelope::Sealed(s) => {
                t.tags.insert(s.tag);
            }
            Envelope::Open(o) => {
                if o.practice_id != tenant_id {
                    self.metrics.dropped_invalid += 1;
                    return true;
                }
                match apply_revocation(&tenant_id, t, self.store.as_ref(), bytes, now_ms) {
                    Some(true) => self.metrics.revocations += 1,
                    Some(false) => {}
                    None => {
                        if !t.pending_revocations.iter().any(|p| p == bytes) {
                            if t.pending_revocations.len() >= MAX_PENDING_REVOCATIONS {
                                t.pending_revocations.remove(0);
                            }
                            t.pending_revocations.push(bytes.to_vec());
                        }
                    }
                }
            }
        }
        let cursor = t.next_cursor;
        t.next_cursor += 1;
        t.buffer.push_back(Buffered {
            cursor,
            at_ms: now_ms,
            from,
            bytes: bytes.to_vec(),
        });
        if let Some(st) = &self.store {
            st.put_envelope(
                &tenant_id,
                &StoredEnvelope {
                    cursor,
                    at_ms: now_ms,
                    from,
                    bytes: bytes.to_vec(),
                },
            );
        }
        t.notify.notify_waiters();
        for (id, s) in &self.sessions {
            if id == session || s.tenant != tenant_id {
                continue;
            }
            if let Some(tx) = &s.tx {
                let _ = tx.send(Outgoing::Frame(Frame::Envelope(bytes.to_vec())));
                self.metrics.envelopes_forwarded += 1;
            }
        }
        self.prune_buffer(tenant_id, now_ms);
        self.enforce_revocations(tenant_id, now_ms);
        true
    }

    /// Closes sessions of devices that are no longer valid members.
    fn enforce_revocations(&mut self, tenant: PracticeId, now_ms: u64) {
        let Some(t) = self.tenants.get(&tenant) else {
            return;
        };
        let gone: Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.tenant == tenant && !t.trust.is_member(&s.device, now_ms))
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            if let Some(s) = self.sessions.get(&id)
                && let Some(tx) = &s.tx
            {
                let _ = tx.send(Outgoing::Close(CLOSE_REVOKED));
            }
            self.disconnect(&id);
        }
    }

    fn prune_buffer(&mut self, tenant: PracticeId, now_ms: u64) {
        let max_age = self.limits.buffer_hours * 60 * 60 * 1000;
        let max_len = self.limits.buffer_max_envelopes;
        if let Some(t) = self.tenants.get_mut(&tenant) {
            let mut popped = false;
            while t
                .buffer
                .front()
                .is_some_and(|b| now_ms.saturating_sub(b.at_ms) > max_age)
                || t.buffer.len() > max_len
            {
                t.buffer.pop_front();
                popped = true;
            }
            if popped && let Some(st) = &self.store {
                let min = t.buffer.front().map_or(t.next_cursor, |b| b.cursor);
                st.prune(&tenant, min);
            }
        }
    }

    /// Long-poll: envelopes after `cursor` (not the session's own), the
    /// newest cursor, and the presence. `None` if the session is unknown or
    /// expired.
    pub fn poll(&mut self, session: &SessionId, cursor: u64, now_ms: u64) -> Option<Polled> {
        let s = self.sessions.get_mut(session)?;
        if s.expires_ms.is_some_and(|e| now_ms > e) {
            self.disconnect(session);
            return None;
        }
        s.last_active_ms = now_ms;
        let (tenant, device) = (s.tenant, s.device);
        let presence = self.presence_of(tenant, Some(*session));
        let t = self.tenants.get(&tenant)?;
        let envelopes = t
            .buffer
            .iter()
            .filter(|b| b.cursor > cursor && b.from != device)
            .map(|b| b.bytes.clone())
            .collect();
        Some(Polled {
            envelopes,
            cursor: t.next_cursor - 1,
            presence,
            notify: t.notify.clone(),
        })
    }

    pub fn tenant_name(&self, id: &PracticeId) -> Option<&str> {
        self.tenants.get(id).map(|t| t.name.as_str())
    }
}

/// The result of a long-poll check.
#[derive(Debug)]
pub struct Polled {
    pub envelopes: Vec<Vec<u8>>,
    pub cursor: u64,
    pub presence: Vec<DeviceId>,
    pub notify: std::sync::Arc<tokio::sync::Notify>,
}

/// Applies an open Revocation envelope signed by one of the tenant's
/// admins. `Some(added)` once verified (or if it is not a valid
/// revocation at all: `Some(false)`); `None` if the signer's cert is not
/// known yet, so it can be retried.
fn apply_revocation(
    tenant: &PracticeId,
    t: &mut Tenant,
    store: Option<&Store>,
    envelope: &[u8],
    now_ms: u64,
) -> Option<bool> {
    let Ok(Envelope::Open(o)) = Envelope::from_cbor(envelope) else {
        return Some(false);
    };
    let Ok(signed) = SignedMessage::from_cbor(&o.payload) else {
        return Some(false);
    };
    let Ok(body) = verify_signed(&signed, true) else {
        return Some(false);
    };
    let Payload::Revocation(r) = &body.payload else {
        return Some(false);
    };
    if body.msg_type() != MsgType::Revocation {
        return Some(false);
    }
    if !t.trust.is_admin(&body.sender, now_ms) {
        // Unknown signer: perhaps an admin that has not used this relay.
        return if t.trust.is_member(&body.sender, now_ms) {
            Some(false)
        } else {
            None
        };
    }
    let rec = RevocationRecord {
        msg_id: body.msg_id,
        target_serial: r.target_serial,
        target_device: r.target_device,
        cascade: r.cascade,
        at: body.created_at,
    };
    let added = t.trust.add_revocation(rec.clone());
    if added && let Some(st) = store {
        st.put_revocation(tenant, &rec);
    }
    Some(added)
}

#[cfg(test)]
mod tests;
