// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Administration and enrolment flows run inside the core: interactive
//! join (flow C), revocation and key rotation, practice settings.

use std::net::SocketAddrV4;

use muster_crypto::cert::issue_cert;
use muster_crypto::{GroupKey, TrustStore, join_code, make_keyshare, open_keyshare};
use muster_proto::cert::CERT_VERSION;
use muster_proto::{
    Body, Cbor, Cert, CertBody, CertSerial, DeviceId, JoinApproval, JoinRequest, Payload,
    PracticeConfig, PracticeId, PracticeSettings, Revocation, Role,
};

use super::{Core, Joining};
use crate::io::{AuditEvent, UdpDest, UiEvent};
use crate::persist::{PersistOp, PracticeInfo, StoredGroupKey};
use crate::time::*;

/// A verified approval waiting for the user's "Join <practice>?" answer.
#[derive(Debug, Clone)]
pub(crate) struct PendingApproval {
    pub info: PracticeInfo,
    pub certs: Vec<Vec<u8>>,
    pub own_serial: CertSerial,
    pub epoch: u64,
    pub key: GroupKey,
    pub config: Vec<u8>,
}

impl Core {
    // ---- Joining device -----------------------------------------------

    pub(crate) fn start_join(&mut self, now: Mono, wall: WallMs, hint: Option<PracticeId>) {
        if self.practice.is_some() {
            return;
        }
        self.joining = Some(Joining {
            started: now,
            next: now,
            hint,
            approval: None,
        });
        let code = join_code(&self.me.0, &self.keys.kx_pub());
        self.ui(UiEvent::JoinCode(code));
        self.send_join_request(now, wall);
    }

    pub(crate) fn send_join_request(&mut self, now: Mono, wall: WallMs) {
        let Some(j) = self.joining.as_mut() else {
            return;
        };
        if now.since(j.started) > JOIN_MAX || j.approval.is_some() {
            j.next = now + JOIN_MAX;
            if now.since(j.started) > JOIN_MAX {
                self.joining = None;
            }
            return;
        }
        j.next = now + JOIN_RESEND;
        let hint = j.hint;
        let req = JoinRequest {
            kx_pub: self.keys.kx_pub(),
            label: self.cfg.label.clone(),
            practice_hint: hint,
            code: join_code(&self.me.0, &self.keys.kx_pub()),
            tcp_port: Some(self.cfg.port),
        };
        let body = self.body(wall, Payload::JoinRequest(req));
        let pid = hint.unwrap_or(PracticeId::ANY);
        let env = muster_crypto::open_envelope(pid, &muster_crypto::sign_body(&self.keys, &body))
            .to_cbor();
        self.send_lan_group(std::slice::from_ref(&env));
        for a in self.cfg.seed_addrs.clone() {
            self.emit(crate::io::Output::SendDatagram {
                dest: UdpDest::Unicast(a),
                bytes: env.clone(),
            });
        }
    }

    pub(crate) fn on_join_approval(
        &mut self,
        now: Mono,
        wall: WallMs,
        practice_id: PracticeId,
        body: &Body,
        ap: JoinApproval,
    ) {
        let Some(j) = &self.joining else { return };
        if j.approval.is_some() || ap.recipient != self.me {
            return;
        }
        // Verify the chain to the root key in the approval (TOFU: the user
        // confirms the practice name next).
        let mut t = TrustStore::new(practice_id, ap.root_pub);
        let mut own_serial = None;
        for raw in &ap.certs {
            let Ok(c) = Cert::from_cbor(raw) else { return };
            match t.add_cert(c) {
                Ok((s, _)) => {
                    if t.cert(&s).is_some_and(|(_, b)| b.subject == self.me) {
                        own_serial = Some(s);
                    }
                }
                Err(_) => return,
            }
        }
        let Some(own_serial) = own_serial else { return };
        if t.validate(&own_serial, wall).is_err() || !t.is_admin(&body.sender, wall) {
            return;
        }
        let Ok(key) = open_keyshare(&self.keys, &practice_id, &ap.keyshare) else {
            return;
        };
        if let Some(j) = self.joining.as_mut() {
            j.approval = Some(PendingApproval {
                info: PracticeInfo {
                    practice_id,
                    root_pub: ap.root_pub,
                    name: ap.practice_name.clone(),
                },
                certs: ap.certs,
                own_serial,
                epoch: ap.keyshare.epoch,
                key,
                config: ap.config,
            });
        }
        self.ui(UiEvent::JoinApproved {
            practice_name: ap.practice_name,
        });
        let _ = now;
    }

    pub(crate) fn confirm_join(&mut self, now: Mono, wall: WallMs) {
        let Some(j) = self.joining.take() else { return };
        let Some(a) = j.approval else {
            self.joining = Some(Joining {
                approval: None,
                ..j
            });
            return;
        };
        self.install_practice(a.info.clone(), Some(a.own_serial));
        self.persist(PersistOp::SetPractice(a.info.clone()));
        self.persist(PersistOp::SetOwnSerial(a.own_serial));
        if let Some(t) = self.trust.as_mut() {
            for raw in &a.certs {
                if let Ok(c) = Cert::from_cbor(raw) {
                    let _ = t.add_cert(c);
                }
            }
        }
        for raw in &a.certs {
            self.persist(PersistOp::AddCert(raw.clone()));
        }
        self.add_group_key(a.epoch, a.key.clone(), wall);
        self.persist(PersistOp::AddGroupKey(StoredGroupKey {
            epoch: a.epoch,
            key: a.key,
            at: wall,
        }));
        if !a.config.is_empty() {
            self.receive(
                now,
                wall,
                &a.config,
                muster_proto::PathKind::Sync,
                None,
                None,
                false,
            );
        }
        self.log(AuditEvent::Enrolled {
            practice_name: a.info.name.clone(),
        });
        self.ui(UiEvent::Enrolled {
            practice_name: a.info.name,
        });
        self.on_wake(now, wall);
    }

    // ---- Admin device -------------------------------------------------

    fn require_admin(&self, wall: WallMs) -> bool {
        self.trust
            .as_ref()
            .is_some_and(|t| t.is_admin(&self.me, wall))
    }

    pub(crate) fn approve_join(&mut self, now: Mono, wall: WallMs, device: DeviceId) {
        if !self.require_admin(wall) {
            return;
        }
        let Some(jr) = self.join_requests.remove(&device) else {
            return;
        };
        let Some(p) = self.practice.clone() else {
            return;
        };
        let Some((epoch, key)) = self.current.clone() else {
            return;
        };
        let serial = CertSerial(self.rng.bytes());
        let cert = issue_cert(
            &self.keys,
            CertBody {
                version: CERT_VERSION,
                practice_id: p.practice_id,
                serial,
                subject: device,
                subject_kx: jr.req.kx_pub,
                role: Role::Member,
                label: jr.req.label.clone(),
                issued_at: wall,
                expires_at: 0,
                issuer: [0; 32],
                token_id: None,
            },
        );
        let raw = cert.to_cbor();
        if let Some(t) = self.trust.as_mut() {
            let _ = t.add_cert(cert);
        }
        self.persist(PersistOp::AddCert(raw.clone()));
        let Ok(ks) = make_keyshare(
            self.rng.bytes(),
            self.rng.bytes(),
            &p.practice_id,
            device,
            &jr.req.kx_pub,
            epoch,
            &key,
        ) else {
            return;
        };
        let mut certs = vec![raw.clone()];
        if let Some(chain) = self
            .trust
            .as_ref()
            .and_then(|t| t.chain_for(&self.me, wall))
        {
            certs.extend(chain.iter().map(|c| c.to_cbor()));
        }
        let ap = JoinApproval {
            recipient: device,
            certs,
            root_pub: p.root_pub,
            practice_name: p.name.clone(),
            keyshare: ks,
            config: self
                .config_msg
                .as_ref()
                .map(|c| c.envelope.clone())
                .unwrap_or_default(),
        };
        let body = self.body(wall, Payload::JoinApproval(ap));
        let env = self.open_env(&body);
        self.processed.insert(body.msg_id, wall);
        let envs = vec![env];
        if let Some(from) = jr.from {
            self.emit(crate::io::Output::SendDatagram {
                dest: UdpDest::Unicast(from),
                bytes: envs[0].clone(),
            });
            let port = jr.req.tcp_port.unwrap_or(self.cfg.port);
            self.emit(crate::io::Output::SendStream {
                to: SocketAddrV4::new(*from.ip(), port),
                bytes: envs[0].clone(),
            });
        }
        if envs[0].len() <= muster_proto::limits::UDP_MAX {
            self.send_lan_group(&envs);
        }
        self.emit(crate::io::Output::SendRelay {
            relay: None,
            bytes: envs[0].clone(),
        });
        // Tell the practice about the new member.
        let ann = self.body(
            wall,
            Payload::CertAnnounce(muster_proto::CertAnnounce { certs: vec![raw] }),
        );
        self.processed.insert(ann.msg_id, wall);
        self.reflooded.insert(ann.msg_id, now);
        let envs = self.seal_all(now, wall, &ann);
        self.flood_new(now, wall, &envs);
        self.log(AuditEvent::CertAdded {
            device,
            role: Role::Member,
        });
    }

    /// Revokes a device and rotates the group key (`spec/04-security.md`,
    /// Revocation and key rotation).
    pub(crate) fn revoke(
        &mut self,
        now: Mono,
        wall: WallMs,
        device: DeviceId,
        cascade: bool,
        reason: &str,
    ) {
        if !self.require_admin(wall) || device == self.me {
            return;
        }
        let Some(p) = self.practice.clone() else {
            return;
        };
        let Some(t) = &self.trust else { return };
        let serial = t
            .certs()
            .find(|(_, b)| b.subject == device)
            .map(|(_, b)| b.serial)
            .unwrap_or_default();
        let new_epoch = self.current_epoch().unwrap_or(0) + 1;
        let r = Revocation {
            target_serial: serial,
            target_device: device,
            cascade,
            reason: reason.to_owned(),
            new_epoch,
        };
        let body = self.body(wall, Payload::Revocation(r.clone()));
        let env = self.open_env(&body);
        self.processed.insert(body.msg_id, wall);
        self.reflooded.insert(body.msg_id, now);
        self.apply_revocation(now, wall, &body, r, &env, false);
        self.flood_new(now, wall, std::slice::from_ref(&env));
        self.rotate(now, wall, p.practice_id, new_epoch);
    }

    fn rotate(&mut self, now: Mono, wall: WallMs, practice_id: PracticeId, epoch: u64) {
        let key = GroupKey::from_bytes(self.rng.bytes());
        let members: Vec<(DeviceId, [u8; 32])> = match &self.trust {
            Some(t) => t
                .members(wall)
                .into_iter()
                .filter(|d| *d != self.me)
                .filter_map(|d| {
                    t.valid_certs(&d, wall)
                        .iter()
                        .map(|c| c.subject_kx)
                        .find(|k| *k != [0; 32])
                        .map(|k| (d, k))
                })
                .collect(),
            None => Vec::new(),
        };
        for (d, kx) in members {
            let Ok(ks) = make_keyshare(
                self.rng.bytes(),
                self.rng.bytes(),
                &practice_id,
                d,
                &kx,
                epoch,
                &key,
            ) else {
                continue;
            };
            let body = self.body(wall, Payload::KeyShare(ks));
            let env = self.open_env(&body);
            self.processed.insert(body.msg_id, wall);
            self.reflooded.insert(body.msg_id, now);
            self.keyshare_envs
                .insert(body.msg_id, (d, epoch, env.clone()));
            self.persist(PersistOp::AddKeyShare(env.clone()));
            let envs = vec![env];
            self.send_direct(now, &d, &envs, true);
            self.flood_new(now, wall, &envs);
            self.keyshare_sent.insert(d, now);
        }
        self.add_group_key(epoch, key.clone(), wall);
        self.persist(PersistOp::AddGroupKey(StoredGroupKey {
            epoch,
            key,
            at: wall,
        }));
        self.next_heartbeat = now;
    }

    pub(crate) fn publish_settings(&mut self, now: Mono, wall: WallMs, settings: PracticeSettings) {
        if !self.require_admin(wall) {
            return;
        }
        let version = self.config_msg.as_ref().map(|c| c.version).unwrap_or(0) + 1;
        let body = self.body(
            wall,
            Payload::PracticeConfig(PracticeConfig { version, settings }),
        );
        let envs = self.seal_all(now, wall, &body);
        let Some(env) = envs.first().cloned() else {
            return;
        };
        // Apply locally through the normal path (quiet: it is our own
        // message), persist, then flood.
        self.receive(
            now,
            wall,
            &env,
            muster_proto::PathKind::Sync,
            None,
            None,
            true,
        );
        self.persist(PersistOp::SetConfig(env.clone()));
        self.reflooded.insert(body.msg_id, now);
        self.flood_new(now, wall, &envs);
    }
}
