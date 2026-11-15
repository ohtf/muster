// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The receive pipeline (`spec/03-protocol.md`, Receiving an alarm or alarm
//! event).

use std::net::SocketAddrV4;

use panic_button_crypto::{
    GroupKey, RevocationRecord, TrustStore, open, open_keyshare, verify_signed,
};
use panic_button_proto::{
    AlarmEvent, AlarmRaised, Body, Cbor, Cert, CertAnnounce, CertSerial, DeviceId, Envelope,
    EventDetail, EventKind, Heartbeat, MsgId, MsgType, PathKind, Payload, PracticeConfig,
    PracticeId, PresenceSummary, Revocation, SignedMessage, SyncRequest, SyncResponse, want,
};

use super::{AlarmEntry, ConfigMsg, Core, JoinReq, Pending, Reflood};
use crate::alarm::{self, EventRec, RaisedRec};
use crate::io::{AuditEvent, DropReason, RelayId, UiEvent};
use crate::persist::{PersistOp, PracticeInfo};
use crate::time::*;

/// Where an envelope came from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Arrival {
    pub path: PathKind,
    pub from: Option<SocketAddrV4>,
    pub relay: Option<RelayId>,
}

fn env_hash(bytes: &[u8]) -> [u8; 16] {
    let h = blake3::hash(bytes);
    let mut o = [0u8; 16];
    o.copy_from_slice(&h.as_bytes()[..16]);
    o
}

impl Core {
    pub(crate) fn install_practice(&mut self, p: PracticeInfo, own_serial: Option<CertSerial>) {
        self.trust = Some(TrustStore::new(p.practice_id, p.root_pub));
        self.practice = Some(p);
        self.own_serial = own_serial;
    }

    /// Adds a group key. Switches the sending key if this is a newer epoch,
    /// or the deterministic winner of a concurrent rotation.
    pub(crate) fn add_group_key(&mut self, epoch: u64, key: GroupKey, wall: WallMs) -> bool {
        let tag = key.tag();
        if self.keys_by_tag.contains_key(&tag) {
            return false;
        }
        self.keys_by_tag.insert(tag, (epoch, key.clone()));
        self.epoch_since.entry(epoch).or_insert(wall);
        let better = match &self.current {
            None => true,
            Some((e, k)) => epoch > *e || (epoch == *e && key.rotation_rank() > k.rotation_rank()),
        };
        if better {
            self.current = Some((epoch, key));
            self.log(AuditEvent::EpochChanged { epoch });
        }
        true
    }

    pub(crate) fn receive_alarm_restore(
        &mut self,
        now: crate::time::Mono,
        wall: WallMs,
        env: &[u8],
    ) {
        self.receive(now, wall, env, PathKind::Sync, None, None, true);
    }

    /// Processes one envelope. `quiet` re-processes stored state at start-up
    /// (no sends, no alerts).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn receive(
        &mut self,
        now: Mono,
        wall: WallMs,
        bytes: &[u8],
        path: PathKind,
        from: Option<SocketAddrV4>,
        relay: Option<RelayId>,
        quiet: bool,
    ) {
        let arrival = Arrival { path, from, relay };
        let h = env_hash(bytes);
        if let Some((id, tracker, _)) = self.env_seen.get(&h).copied() {
            // Exactly the same envelope again: already verified. A duplicate
            // still updates path tracking (`spec/03-protocol.md`, step 3).
            if let Some(sender) = tracker
                && sender != self.me
                && self
                    .trust
                    .as_ref()
                    .is_some_and(|t| t.is_member(&sender, wall))
            {
                self.peers.heard(sender, path, from, now, wall);
            }
            self.on_duplicate(now, wall, id, arrival);
            return;
        }
        let env = match Envelope::from_cbor(bytes) {
            Ok(e) => e,
            Err(panic_button_proto::DecodeError::UnsupportedVersion(_)) => {
                return self.drop_msg(DropReason::UnsupportedVersion);
            }
            Err(_) => return self.drop_msg(DropReason::Decode),
        };
        match env {
            Envelope::Sealed(s) => {
                // Cheap tag check before any crypto (FR-NET-004).
                let Some((epoch, key)) = self.keys_by_tag.get(&s.tag).cloned() else {
                    return self.drop_msg(DropReason::UnknownTag);
                };
                let signed = match open(&key, &s) {
                    Ok(m) => m,
                    Err(_) => return self.drop_msg(DropReason::DecryptFailed),
                };
                let body = match verify_signed(&signed, false) {
                    Ok(b) => b,
                    Err(panic_button_crypto::CryptoError::Decode(
                        panic_button_proto::DecodeError::UnknownType(_),
                    )) => return self.drop_msg(DropReason::UnknownType),
                    Err(_) => return self.drop_msg(DropReason::BadSignature),
                };
                let _ = epoch;
                let tracker = matches!(
                    body.msg_type(),
                    MsgType::Heartbeat | MsgType::PresenceSummary
                )
                .then_some(body.sender);
                self.env_seen.insert(h, (body.msg_id, tracker, now));
                self.on_body(now, wall, body, bytes, arrival, quiet);
            }
            Envelope::Open(o) => {
                let signed = match SignedMessage::from_cbor(&o.payload) {
                    Ok(s) => s,
                    Err(_) => return self.drop_msg(DropReason::Decode),
                };
                let body = match verify_signed(&signed, true) {
                    Ok(b) => b,
                    Err(_) => return self.drop_msg(DropReason::BadSignature),
                };
                self.env_seen.insert(h, (body.msg_id, None, now));
                self.on_open(now, wall, o.practice_id, body, bytes, arrival, quiet);
            }
        }
    }

    /// A message we have already processed arrived again.
    fn on_duplicate(&mut self, now: Mono, wall: WallMs, id: MsgId, a: Arrival) {
        // Re-flood suppression: someone else covered this segment.
        if matches!(a.path, PathKind::Multicast | PathKind::Broadcast)
            && let Some(r) = self.refloods.get_mut(&id)
        {
            r.heard_lan = true;
            if a.from.is_some() && a.from != r.first_from && !r.repeat {
                r.suppressed = true;
            }
        }
        if let Some(from) = a.from
            && let Some(d) = self.peers.by_udp_addr(&from)
        {
            self.known_has.entry(id).or_default().insert(d);
        }
        // An origin unicasting a retransmission to us means it lacks our
        // receipt: send it again directly, rate-limited (ADR 0013).
        if matches!(a.path, PathKind::UdpUnicast | PathKind::Tcp)
            && let Some(e) = self.alarms.get(&id)
            && let Some(env) = e.receipt_env.clone()
            && e.last_receipt_send
                .is_none_or(|t| now.since(t) >= RECEIPT_RESEND_MIN)
            && e.msgs.derive().closed.is_none()
        {
            let origin = e.msgs.raised.as_ref().map(|r| r.origin);
            if let Some(en) = self.alarms.get_mut(&id) {
                en.last_receipt_send = Some(now);
            }
            if let Some(o) = origin {
                self.send_direct(now, &o, std::slice::from_ref(&env), true);
            }
        }
        let _ = wall;
    }

    /// Pre-dispatch checks shared by sealed messages.
    fn on_body(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: Body,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        if body.sender == self.me && !quiet {
            // Our own multicast looped back, or our message re-flooded.
            if let Some(r) = self.refloods.get_mut(&body.msg_id)
                && matches!(a.path, PathKind::Multicast | PathKind::Broadcast)
            {
                r.suppressed = true;
            }
            return;
        }
        let t = body.msg_type();
        if self.processed.contains_key(&body.msg_id) {
            return self.on_duplicate(now, wall, body.msg_id, a);
        }
        // Certificate check. Self-authenticating content is exempt, as is a
        // request for certs and revocations only: answering it reveals
        // nothing a group-key holder could not already see, and without it
        // two devices that each lack the other's cert would deadlock.
        let cert_request = matches!(
            &body.payload,
            Payload::SyncRequest(r)
                if r.ids.is_empty() && r.want & !(want::CERTS | want::REVOCATIONS) == 0
        );
        let exempt = matches!(t, MsgType::CertAnnounce | MsgType::SyncResponse) || cert_request;
        if !exempt {
            let Some(trust) = &self.trust else { return };
            if !trust.is_member(&body.sender, wall) {
                if trust.is_device_revoked(&body.sender) {
                    return self.drop_msg(DropReason::Revoked);
                }
                self.hold_pending(now, wall, bytes, a, &body);
                return;
            }
        }
        self.mark_processed(&body, wall, quiet);
        match body.payload.clone() {
            Payload::Heartbeat(hb) => self.on_heartbeat(now, wall, &body, hb, a),
            Payload::AlarmRaised(p) => self.on_alarm_raised(now, wall, &body, p, bytes, a, quiet),
            Payload::AlarmEvent(p) => self.on_alarm_event(now, wall, &body, p, bytes, a, quiet),
            Payload::SyncRequest(r) => {
                self.peers.heard(body.sender, a.path, a.from, now, wall);
                if !quiet {
                    self.on_sync_request(now, wall, &body, r, a);
                }
            }
            Payload::SyncResponse(r) => self.on_sync_response(now, wall, r, a, quiet),
            Payload::CertAnnounce(c) => self.on_cert_announce(now, wall, &body, c, bytes, a, quiet),
            Payload::PracticeConfig(c) => self.on_config(now, wall, &body, c, bytes, a, quiet),
            Payload::PresenceSummary(p) => self.on_presence(now, wall, &body, p, a),
            Payload::DeviceSettings(d) => {
                self.on_device_settings(now, wall, &body, d, bytes, a, quiet)
            }
            _ => self.drop_msg(DropReason::UnknownType),
        }
    }

    fn mark_processed(&mut self, body: &Body, wall: WallMs, quiet: bool) {
        self.processed.insert(body.msg_id, wall);
        let t = body.msg_type();
        if !quiet && matches!(t, MsgType::AlarmRaised | MsgType::AlarmEvent) {
            self.persist(PersistOp::Processed {
                msg_id: body.msg_id,
                at: wall,
            });
        }
        self.known_has
            .entry(body.msg_id)
            .or_default()
            .insert(body.sender);
    }

    fn hold_pending(&mut self, now: Mono, wall: WallMs, bytes: &[u8], a: Arrival, body: &Body) {
        // Not processed yet: a later copy (by sync, say) must not be taken
        // for a duplicate.
        self.env_seen.remove(&env_hash(bytes));
        // Record the path now (the signature and group key check out), so the
        // peer is classed correctly once its cert arrives.
        if body.msg_type() == MsgType::Heartbeat {
            self.peers.heard(body.sender, a.path, a.from, now, wall);
        }
        // Keep only the latest heartbeat per sender (its addresses matter if
        // the path it came by fails before the next one); drop repeated
        // presence and sync requests.
        let keep = !matches!(
            body.msg_type(),
            MsgType::PresenceSummary | MsgType::SyncRequest
        );
        if body.msg_type() == MsgType::Heartbeat {
            self.pending
                .retain(|p| !(p.sender == body.sender && p.heartbeat));
        }
        if keep {
            if self.pending.len() >= 1000 {
                self.drop_msg(DropReason::RateLimited);
            } else if !self.pending.iter().any(|p| p.bytes == bytes) {
                self.pending.push(Pending {
                    sender: body.sender,
                    heartbeat: body.msg_type() == MsgType::Heartbeat,
                    bytes: bytes.to_vec(),
                    path: a.path,
                    from: a.from,
                    relay: a.relay,
                    until: now + PENDING_CERT_WAIT,
                });
            }
        }
        // Certs usually arrive by flooding within moments, so wait before
        // asking (alarms ask at once).
        let urgent = matches!(body.msg_type(), MsgType::AlarmRaised | MsgType::AlarmEvent);
        // Remember when we first wanted it, but ask by the latest path.
        let entry = self.cert_wanted.entry(body.sender).or_insert((now, a));
        entry.1 = a;
        let first = entry.0;
        if !urgent && now.since(first) < CERT_REQUEST_DELAY {
            return;
        }
        self.request_certs(now, wall, body.sender, a);
    }

    pub(crate) fn request_certs(&mut self, now: Mono, wall: WallMs, sender: DeviceId, a: Arrival) {
        // Back off 5 s, 10 s, 20 s, then every 30 s: a lost request on a
        // lossy link must not leave a device unheard for long.
        if let Some((t, n)) = self.cert_sync_sent.get(&sender).copied() {
            let gap = ((5 * SECOND) << n.min(3)).min(SYNC_RATE_LIMIT);
            if now.since(t) < gap {
                return;
            }
        }
        let attempts = self
            .cert_sync_sent
            .get(&sender)
            .map(|(_, n)| n + 1)
            .unwrap_or(0);
        self.cert_sync_sent.insert(sender, (now, attempts));
        // Ask whoever passed the message on. Only a request broadcast via a
        // relay needs a named responder; the signer has its own cert.
        let req = SyncRequest {
            want: want::CERTS | want::REVOCATIONS,
            since: None,
            ids: vec![],
            to: (a.path == PathKind::Relay).then_some(sender),
            alarm_digest: None,
        };
        self.send_sync_request_via(now, wall, req, a);
        // And introduce ourselves the same way, in case they lack our cert.
        self.send_own_chain_via(now, wall, a);
    }

    /// Asks for certs of senders still unknown after the delay.
    pub(crate) fn run_cert_requests(&mut self, now: Mono, wall: WallMs) {
        let Some(t) = &self.trust else { return };
        self.cert_wanted
            .retain(|d, _| !t.is_member(d, wall) && !t.is_device_revoked(d));
        let due: Vec<(DeviceId, Arrival)> = self
            .cert_wanted
            .iter()
            .filter(|(_, (t0, _))| now.since(*t0) >= CERT_REQUEST_DELAY)
            .map(|(d, (_, a))| (*d, *a))
            .collect();
        for (d, a) in due {
            self.request_certs(now, wall, d, a);
        }
    }

    /// Re-runs pending messages after new certs arrive.
    pub(crate) fn retry_pending(&mut self, now: Mono, wall: WallMs) {
        let Some(trust) = &self.trust else { return };
        let (ready, waiting): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| trust.is_member(&p.sender, wall) || trust.is_device_revoked(&p.sender));
        self.pending = waiting;
        for p in ready {
            let h = env_hash(&p.bytes);
            self.env_seen.remove(&h);
            let before = self.pending.len();
            self.receive(now, wall, &p.bytes, p.path, p.from, p.relay, false);
            // If it went back into pending, keep the original deadline.
            if self.pending.len() > before
                && let Some(last) = self.pending.last_mut()
            {
                last.until = p.until;
            }
        }
    }

    fn on_heartbeat(&mut self, now: Mono, wall: WallMs, body: &Body, hb: Heartbeat, a: Arrival) {
        let sender = body.sender;
        self.peers.heard(sender, a.path, a.from, now, wall);
        if body.created_at > wall.saturating_add(FUTURE_SKEW_LIMIT)
            && self.skew_logged.insert(sender)
        {
            self.log(AuditEvent::ClockSkew {
                device: sender,
                ahead_ms: body.created_at - wall,
            });
        }
        let their_epoch = hb.epoch;
        let (ad, sd) = (hb.alarm_digest, hb.state_digest);
        if self.pending_settings.contains_key(&sender) {
            let (l, r) = (hb.label.clone(), hb.room.clone());
            self.check_pending_settings(now, sender, &l, &r);
        }
        self.peers.heartbeat(sender, hb);
        let Some(cur) = self.current_epoch() else {
            return;
        };
        if their_epoch < cur {
            self.send_keyshare_to(now, wall, sender);
        } else if their_epoch > cur {
            self.higher_epoch_seen = Some(self.higher_epoch_seen.unwrap_or(0).max(their_epoch));
        }
        let (my_ad, my_sd) = self.heartbeat_digests(wall);
        let mut w = 0;
        // Digests differ transiently while messages are in flight, so sync
        // only when a mismatch persists for two heartbeats. Retransmission
        // and re-flooding deliver alarms; this is the backstop.
        // Peers off the LAN group are few and their links are the ones that
        // lose messages, so they sync at the first mismatch.
        let lan = self
            .peers
            .get(&sender)
            .is_some_and(|p| p.class(now) == crate::peers::PeerClass::LanGroup);
        if ad != my_ad {
            if !self.ad_mismatch.insert(sender) || !lan {
                w |= want::OPEN_ALARMS | want::RECENT_ALARM_EVENTS;
            }
        } else {
            self.ad_mismatch.remove(&sender);
        }
        // State (certs, revocations, config) converges by flooding; only
        // sync if the mismatch persists for two heartbeats. Alarm state
        // mismatches sync at once.
        if sd != my_sd {
            if !self.sd_mismatch.insert(sender) {
                w |= want::CERTS | want::REVOCATIONS | want::CONFIG | want::KEYSHARES_FOR_ME;
            }
        } else {
            self.sd_mismatch.remove(&sender);
        }
        let limit = if lan {
            SYNC_RATE_LIMIT
        } else {
            SYNC_RATE_LIMIT_NON_LAN
        };
        if w != 0
            && self
                .sync_sent
                .get(&sender)
                .is_none_or(|t| now.since(*t) >= limit)
        {
            self.sync_sent.insert(sender, now);
            let req = SyncRequest {
                want: w,
                since: None,
                ids: vec![],
                to: Some(sender),
                alarm_digest: Some(my_ad),
            };
            self.send_sync_request_to(now, wall, sender, req);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_alarm_raised(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        p: AlarmRaised,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        let id = body.msg_id;
        if p.alarm_id != id {
            return self.drop_msg(DropReason::Decode);
        }
        let entry = self.alarms.entry(id).or_default();
        if entry.msgs.raised.is_some() {
            return;
        }
        let first_time = !entry.known;
        entry.known = true;
        if first_time && entry.local.first_seen == 0 {
            entry.local.first_seen = wall;
        }
        entry.msgs.raised = Some(RaisedRec {
            created_at: body.created_at,
            origin: body.sender,
            payload: p.clone(),
        });
        entry.envelopes.insert(id, bytes.to_vec());
        if quiet {
            return;
        }
        let (_, skewed) = alarm::alarm_age(body.created_at, wall);
        if skewed && !entry.skew_logged {
            entry.skew_logged = true;
            self.log(AuditEvent::ClockSkew {
                device: body.sender,
                ahead_ms: body.created_at - wall,
            });
        }
        // Re-flood, including raises first learned by sync while the alarm
        // is live: our LAN neighbours may have missed it too.
        let live = alarm::alarm_age(body.created_at, wall).0 <= ALARM_MAX_LIFE;
        if a.path != PathKind::Sync || live {
            self.schedule_reflood(now, id, bytes, a, body.sender);
            if let Some(r) = self.refloods.get_mut(&id) {
                r.repeats_left = REFLOOD_REPEAT.len();
            }
        }
        self.persist(PersistOp::AddAlarmEnvelope {
            alarm_id: id,
            msg_id: id,
            bytes: bytes.to_vec(),
        });
        self.log(AuditEvent::AlarmReceived {
            alarm_id: id,
            origin: body.sender,
            path: a.path,
        });
        if let Some(d) = self.deferred_raises.as_mut() {
            // Inside a sync response: decide once every envelope is applied,
            // so a raise listed before its Resolved event does not alert.
            d.push((id, a.path));
            return;
        }
        self.after_raise(now, wall, id, a.path);
    }

    /// Receipt, alert or history for a newly known raise.
    pub(crate) fn after_raise(&mut self, now: Mono, wall: WallMs, id: MsgId, path: PathKind) {
        let Some(r) = self.alarms.get(&id).and_then(|e| e.msgs.raised.clone()) else {
            return;
        };
        let p = r.payload.clone();
        let (age, _) = alarm::alarm_age(r.created_at, wall);
        let closed = self
            .alarms
            .get(&id)
            .is_some_and(|e| e.msgs.derive().closed.is_some());
        let expired = age > ALARM_MAX_LIFE;
        if !expired && !closed {
            self.send_receipt(now, wall, id, r.origin, p.drill, path);
        }
        if p.drill {
            // Drills never cause a full alert (P12).
            let audible = self.cfg.drill_mode == panic_button_proto::DrillMode::Audible;
            self.ui(UiEvent::DrillToast {
                alarm_id: id,
                audible,
            });
            if audible {
                self.emit(crate::io::Output::Audio(crate::io::AudioCmd::Chime));
            }
            self.history_only(id, wall);
            return;
        }
        if expired || closed {
            // FR-ALM-008, S8: straight to history.
            self.history_only(id, wall);
            return;
        }
        let Some(view) = self
            .alarms
            .get(&id)
            .and_then(|e| alarm::view(id, &e.msgs, &self.me, None, wall))
        else {
            return;
        };
        if let Some(e) = self.alarms.get_mut(&id) {
            e.local.alerted = true;
            e.last_view = Some(view.clone());
        }
        let local = self.alarms.get(&id).map(|e| e.local).unwrap_or_default();
        self.persist(PersistOp::SetAlarmLocal {
            alarm_id: id,
            local,
        });
        self.ui(UiEvent::AlarmShown {
            alarm: view,
            resumed: false,
        });
        self.update_sound();
    }

    fn history_only(&mut self, id: MsgId, wall: WallMs) {
        let Some(e) = self.alarms.get_mut(&id) else {
            return;
        };
        e.local.history_only = true;
        let local = e.local;
        let v = alarm::view(id, &e.msgs, &self.me, e.local.local_ack, wall);
        e.last_view = v.clone();
        self.persist(PersistOp::SetAlarmLocal {
            alarm_id: id,
            local,
        });
        if let Some(v) = v
            && !v.drill
        {
            self.ui(UiEvent::AlarmHistory(v));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_alarm_event(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        p: AlarmEvent,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        let id = p.alarm_id;
        let rec = EventRec {
            msg_id: body.msg_id,
            created_at: body.created_at,
            sender: body.sender,
            kind: p.event,
            user_display: p.user_display.clone(),
            detail: p.detail.clone(),
        };
        let entry: &mut AlarmEntry = self.alarms.entry(id).or_default();
        if entry.local.first_seen == 0 {
            entry.local.first_seen = wall;
        }
        entry.msgs.events.insert(body.msg_id, rec);
        entry.envelopes.insert(body.msg_id, bytes.to_vec());
        if p.event == EventKind::Receipt {
            self.known_has.entry(id).or_default().insert(body.sender);
        }
        if quiet {
            return;
        }
        let have_raise = self
            .alarms
            .get(&id)
            .is_some_and(|e| e.msgs.raised.is_some());
        if a.path != PathKind::Sync {
            self.schedule_reflood(now, body.msg_id, bytes, a, body.sender);
        }
        self.persist(PersistOp::AddAlarmEnvelope {
            alarm_id: id,
            msg_id: body.msg_id,
            bytes: bytes.to_vec(),
        });
        self.log(AuditEvent::AlarmEvent {
            alarm_id: id,
            by: body.sender,
            event: p.event,
        });
        if !have_raise {
            // Hold the event and ask for the raise.
            let ask = self.alarms.get(&id).is_some_and(|e| {
                e.raise_requested
                    .is_none_or(|t| now.since(t) >= SYNC_RATE_LIMIT)
            });
            if ask {
                if let Some(e) = self.alarms.get_mut(&id) {
                    e.raise_requested = Some(now);
                }
                let req = SyncRequest {
                    want: want::OPEN_ALARMS,
                    since: None,
                    ids: vec![id],
                    to: None,
                    alarm_digest: None,
                };
                self.send_sync_request_via(now, wall, req, a);
            }
            return;
        }
        // Drill receipts: RTT measured locally (FR-DRL-004).
        if p.event == EventKind::Receipt
            && let Some(run) = self.drills.get_mut(&id)
        {
            run.rtt.entry(body.sender).or_insert(now.since(run.t0));
        }
        if let EventDetail::Receipt(_) = p.detail {}
        self.refresh_alarm(now, wall, id);
    }

    fn on_sync_request(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        r: SyncRequest,
        a: Arrival,
    ) {
        if r.to.is_some_and(|t| t != self.me) {
            return;
        }
        // The requester's alarm state differs from ours: they may have what
        // we lack, so ask back.
        if let Some(theirs) = r.alarm_digest
            && theirs != self.heartbeat_digests(wall).0
        {
            let lan = self
                .peers
                .get(&body.sender)
                .is_some_and(|p| p.class(now) == crate::peers::PeerClass::LanGroup);
            let limit = if lan {
                SYNC_RATE_LIMIT
            } else {
                SYNC_RATE_LIMIT_NON_LAN
            };
            if self
                .sync_sent
                .get(&body.sender)
                .is_none_or(|t| now.since(*t) >= limit)
            {
                self.sync_sent.insert(body.sender, now);
                let back = SyncRequest {
                    want: want::OPEN_ALARMS | want::RECENT_ALARM_EVENTS,
                    since: None,
                    ids: vec![],
                    to: Some(body.sender),
                    alarm_digest: None,
                };
                self.send_sync_request_via(now, wall, back, a);
            }
        }
        let mut envs: Vec<Vec<u8>> = Vec::new();
        if r.want & (want::OPEN_ALARMS | want::RECENT_ALARM_EVENTS) != 0 || !r.ids.is_empty() {
            for (id, e) in &self.alarms {
                let wanted = if r.ids.is_empty() {
                    let Some(raised) = &e.msgs.raised else {
                        continue;
                    };
                    let closed_at = e.msgs.derive().closed.map(|c| c.at);
                    wall.saturating_sub(raised.created_at) <= ALARM_MAX_LIFE
                        || closed_at.is_some_and(|c| wall.saturating_sub(c) <= ALARM_MAX_LIFE)
                } else {
                    r.ids.contains(id)
                };
                if wanted {
                    envs.extend(e.envelopes.values().cloned());
                }
            }
        }
        if r.want & want::REVOCATIONS != 0 {
            envs.extend(self.revocation_envs.values().cloned());
        }
        if r.want & want::CONFIG != 0
            && let Some(c) = &self.config_msg
        {
            envs.push(c.envelope.clone());
        }
        if r.want & want::KEYSHARES_FOR_ME != 0 {
            envs.extend(
                self.keyshare_envs
                    .values()
                    .filter(|(rcpt, _, _)| *rcpt == body.sender)
                    .map(|(_, _, e)| e.clone()),
            );
        }
        if r.want & want::CERTS != 0 {
            envs.extend(self.cert_announce_envelopes(now, wall));
        }
        self.send_sync_response(now, wall, body.sender, envs, a);
    }

    fn on_sync_response(
        &mut self,
        now: Mono,
        wall: WallMs,
        r: SyncResponse,
        a: Arrival,
        quiet: bool,
    ) {
        if a.path == PathKind::Sync {
            // No nested sync responses.
            return;
        }
        let outer = self.deferred_raises.is_none();
        if outer {
            self.deferred_raises = Some(Vec::new());
        }
        for env in r.envelopes {
            self.receive(now, wall, &env, PathKind::Sync, a.from, a.relay, quiet);
        }
        if outer {
            for (id, path) in self.deferred_raises.take().unwrap_or_default() {
                self.after_raise(now, wall, id, path);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_cert_announce(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        c: CertAnnounce,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        let mut added = false;
        if let Some(t) = self.trust.as_mut() {
            for raw in &c.certs {
                let Ok(cert) = Cert::from_cbor(raw) else {
                    continue;
                };
                if let Ok((_, true)) = t.add_cert(cert) {
                    added = true;
                    if !quiet {
                        self.out
                            .push(crate::io::Output::Persist(PersistOp::AddCert(raw.clone())));
                    }
                }
            }
        }
        if !quiet && added {
            if a.path != PathKind::Sync {
                self.schedule_reflood(now, body.msg_id, bytes, a, body.sender);
            }
            self.retry_pending(now, wall);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_config(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        c: PracticeConfig,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        let admin = self
            .trust
            .as_ref()
            .is_some_and(|t| t.is_admin(&body.sender, wall));
        if !admin {
            return self.drop_msg(DropReason::NotAuthorised);
        }
        let newer = self
            .config_msg
            .as_ref()
            .is_none_or(|old| (c.version, body.msg_id) > (old.version, old.msg_id));
        if !newer {
            return;
        }
        self.config_msg = Some(ConfigMsg {
            version: c.version,
            msg_id: body.msg_id,
            settings: c.settings,
            envelope: bytes.to_vec(),
        });
        self.recompute_config();
        if !quiet {
            self.persist(PersistOp::SetConfig(bytes.to_vec()));
            self.log(AuditEvent::ConfigChanged {
                version: c.version,
                by: body.sender,
            });
            if a.path != PathKind::Sync {
                self.schedule_reflood(now, body.msg_id, bytes, a, body.sender);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_device_settings(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        d: panic_button_proto::DeviceSettings,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        let admin = self
            .trust
            .as_ref()
            .is_some_and(|t| t.is_admin(&body.sender, wall));
        if !admin {
            return self.drop_msg(DropReason::NotAuthorised);
        }
        if d.target == self.me {
            self.apply_own_settings(d.label, d.room, d.assigned_at, Some(body.sender));
        }
        if !quiet && a.path != PathKind::Sync {
            self.schedule_reflood(now, body.msg_id, bytes, a, body.sender);
        }
    }

    fn on_presence(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        p: PresenceSummary,
        a: Arrival,
    ) {
        self.peers.heard(body.sender, a.path, a.from, now, wall);
        for e in p.peers {
            if e.device_id != self.me {
                self.peers.indirect(e.device_id, now);
            }
        }
    }

    pub(crate) fn schedule_reflood(
        &mut self,
        now: Mono,
        msg_id: MsgId,
        bytes: &[u8],
        a: Arrival,
        origin: panic_button_proto::DeviceId,
    ) {
        if self.reflooded.contains_key(&msg_id) || self.refloods.contains_key(&msg_id) {
            return;
        }
        let jitter = self.rng.below_incl(REFLOOD_JITTER);
        self.refloods.insert(
            msg_id,
            Reflood {
                at: now + jitter,
                msg_id,
                envelopes: vec![bytes.to_vec()],
                first_from: a.from,
                arrived_via_relay: a.path == PathKind::Relay,
                suppressed: false,
                origin,
                repeats_left: 0,
                repeat: false,
                heard_lan: matches!(a.path, PathKind::Multicast | PathKind::Broadcast),
            },
        );
    }

    /// Open envelopes: join, approval, key share, revocation.
    #[allow(clippy::too_many_arguments)]
    fn on_open(
        &mut self,
        now: Mono,
        wall: WallMs,
        practice_id: PracticeId,
        body: Body,
        bytes: &[u8],
        a: Arrival,
        quiet: bool,
    ) {
        if body.sender == self.me {
            if let Some(r) = self.refloods.get_mut(&body.msg_id)
                && matches!(a.path, PathKind::Multicast | PathKind::Broadcast)
            {
                r.suppressed = true;
            }
            return;
        }
        if self.processed.contains_key(&body.msg_id) {
            return self.on_duplicate(now, wall, body.msg_id, a);
        }
        let ours = self.practice.as_ref().map(|p| p.practice_id);
        match body.payload.clone() {
            Payload::JoinRequest(r) => {
                if ours.is_none() {
                    return;
                }
                if practice_id != PracticeId::ANY && Some(practice_id) != ours {
                    return self.drop_msg(DropReason::WrongPractice);
                }
                self.processed.insert(body.msg_id, wall);
                let is_admin = self
                    .trust
                    .as_ref()
                    .is_some_and(|t| t.is_admin(&self.me, wall));
                let already_member = self
                    .trust
                    .as_ref()
                    .is_some_and(|t| t.is_member(&body.sender, wall));
                if !is_admin || already_member || quiet {
                    return;
                }
                // Check the code matches the keys, so a relayed request cannot
                // show a code from another device.
                if panic_button_crypto::join_code(&body.sender.0, &r.kx_pub) != r.code {
                    return self.drop_msg(DropReason::BadSignature);
                }
                let new = self
                    .join_requests
                    .get(&body.sender)
                    .is_none_or(|j| j.req.code != r.code);
                self.join_requests.insert(
                    body.sender,
                    JoinReq {
                        req: r.clone(),
                        from: a.from,
                        at: now,
                    },
                );
                if new {
                    self.ui(UiEvent::JoinRequested {
                        device: body.sender,
                        label: r.label,
                        code: r.code,
                    });
                }
            }
            Payload::JoinApproval(ap) => {
                if ours.is_some() || quiet {
                    return;
                }
                self.processed.insert(body.msg_id, wall);
                self.on_join_approval(now, wall, practice_id, &body, ap);
            }
            Payload::KeyShare(ks) => {
                if Some(practice_id) != ours {
                    return self.drop_msg(DropReason::WrongPractice);
                }
                let Some(trust) = &self.trust else { return };
                if !trust.is_member(&body.sender, wall) {
                    if trust.is_device_revoked(&body.sender) {
                        return self.drop_msg(DropReason::Revoked);
                    }
                    return self.hold_pending(now, wall, bytes, a, &body);
                }
                self.processed.insert(body.msg_id, wall);
                self.keyshare_envs
                    .insert(body.msg_id, (ks.recipient, ks.epoch, bytes.to_vec()));
                if !quiet {
                    self.persist(PersistOp::AddKeyShare(bytes.to_vec()));
                }
                if ks.recipient == self.me
                    && let Some(p) = &self.practice
                    && let Ok(key) = open_keyshare(&self.keys, &p.practice_id, &ks)
                    && self.add_group_key(ks.epoch, key.clone(), wall)
                    && !quiet
                {
                    self.persist(PersistOp::AddGroupKey(crate::persist::StoredGroupKey {
                        epoch: ks.epoch,
                        key,
                        at: wall,
                    }));
                    // Tell everyone our new epoch promptly.
                    self.next_heartbeat = now;
                }
                if !quiet && a.path != PathKind::Sync && ks.recipient != self.me {
                    self.schedule_reflood(now, body.msg_id, bytes, a, body.sender);
                }
            }
            Payload::Revocation(r) => {
                if Some(practice_id) != ours {
                    return self.drop_msg(DropReason::WrongPractice);
                }
                let Some(trust) = &self.trust else { return };
                if !trust.is_admin(&body.sender, wall) {
                    if trust.is_device_revoked(&body.sender) {
                        return self.drop_msg(DropReason::Revoked);
                    }
                    if trust.is_member(&body.sender, wall) {
                        return self.drop_msg(DropReason::NotAuthorised);
                    }
                    return self.hold_pending(now, wall, bytes, a, &body);
                }
                self.processed.insert(body.msg_id, wall);
                self.apply_revocation(now, wall, &body, r, bytes, quiet);
                if !quiet && a.path != PathKind::Sync {
                    self.schedule_reflood(now, body.msg_id, bytes, a, body.sender);
                }
            }
            _ => self.drop_msg(DropReason::UnknownType),
        }
    }

    pub(crate) fn apply_revocation(
        &mut self,
        now: Mono,
        wall: WallMs,
        body: &Body,
        r: Revocation,
        bytes: &[u8],
        quiet: bool,
    ) {
        let Some(trust) = self.trust.as_mut() else {
            return;
        };
        let added = trust.add_revocation(RevocationRecord {
            msg_id: body.msg_id,
            target_serial: r.target_serial,
            target_device: r.target_device,
            cascade: r.cascade,
            at: body.created_at,
        });
        self.revocation_envs.insert(body.msg_id, bytes.to_vec());
        self.revocation_epochs.insert(r.new_epoch);
        if !added || quiet {
            return;
        }
        self.persist(PersistOp::AddRevocation(bytes.to_vec()));
        self.log(AuditEvent::Revoked {
            device: r.target_device,
            cascade: r.cascade,
            by: body.sender,
        });
        if r.new_epoch > self.current_epoch().unwrap_or(0) {
            self.higher_epoch_seen = Some(self.higher_epoch_seen.unwrap_or(0).max(r.new_epoch));
        }
        // Drop peers that are no longer members.
        let gone: Vec<_> = self
            .peers
            .iter()
            .map(|p| p.device_id)
            .filter(|d| {
                self.trust
                    .as_ref()
                    .is_some_and(|t| t.is_device_revoked(d) || !t.is_member(d, wall))
            })
            .collect();
        for d in gone {
            if self.trust.as_ref().is_some_and(|t| t.is_device_revoked(&d)) {
                self.peers.remove(&d);
            }
        }
        let _ = now;
    }
}
