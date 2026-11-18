// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

use crate::crypto::{
    DeviceKeys, GroupKey, SigningSecret, issue_cert, open_envelope, seal, sign_body,
};
use crate::proto::cert::CERT_VERSION;
use crate::proto::relay::Frame;
use crate::proto::{
    Body, Cbor, CertBody, CertSerial, DeviceId, MsgId, Payload, PracticeId, RelayAuth, Revocation,
    Role,
};
use tokio::sync::mpsc;

use super::*;
use crate::relay::config::Limits;

const NOW: u64 = 1_763_100_000_000;
const PID: PracticeId = PracticeId([1; 16]);

struct Practice {
    root: SigningSecret,
    admin: DeviceKeys,
    admin_cert: Vec<u8>,
}

fn practice() -> Practice {
    let root = SigningSecret::from_secret([9; 32]);
    let admin = DeviceKeys::from_secrets([1; 32], [2; 32]);
    let admin_cert = issue_cert(&root, body(&admin, Role::Admin, 1)).to_cbor();
    Practice {
        root,
        admin,
        admin_cert,
    }
}

fn body(k: &DeviceKeys, role: Role, serial: u8) -> CertBody {
    CertBody {
        version: CERT_VERSION,
        practice_id: PID,
        serial: CertSerial([serial; 16]),
        subject: k.device_id(),
        subject_kx: k.kx_pub(),
        role,
        label: String::new(),
        issued_at: NOW - 1000,
        expires_at: 0,
        issuer: [0; 32],
        token_id: None,
    }
}

fn member(p: &Practice, n: u8) -> (DeviceKeys, Vec<Vec<u8>>) {
    let k = DeviceKeys::from_secrets([n; 32], [n + 50; 32]);
    let c = issue_cert(&p.admin, body(&k, Role::Member, n)).to_cbor();
    (k, vec![c, p.admin_cert.clone()])
}

fn auth_env(k: &DeviceKeys, chain: Vec<Vec<u8>>, nonce: [u8; 32], resume: Option<u64>) -> Vec<u8> {
    let b = Body {
        msg_id: MsgId([k.device_id().0[0]; 16]),
        created_at: NOW,
        sender: k.device_id(),
        payload: Payload::RelayAuth(RelayAuth {
            practice_id: PID,
            chain,
            nonce,
            client_time: NOW,
            resume_cursor: resume,
        }),
    };
    open_envelope(PID, &sign_body(k, &b)).to_cbor()
}

fn sealed(k: &DeviceKeys, n: u8) -> Vec<u8> {
    let b = Body {
        msg_id: MsgId([n; 16]),
        created_at: NOW,
        sender: k.device_id(),
        payload: Payload::CertAnnounce(crate::proto::CertAnnounce { certs: vec![] }),
    };
    seal(
        &GroupKey::from_bytes([3; 32]),
        0,
        [n; 24],
        &sign_body(k, &b),
    )
    .to_cbor()
}

fn relay(p: &Practice) -> Relay {
    let mut r = Relay::new(Limits::default());
    r.add_tenant(PID, p.root.public(), "Test", NOW);
    r
}

type Rx = mpsc::UnboundedReceiver<Outgoing>;

fn connect(
    r: &mut Relay,
    k: &DeviceKeys,
    chain: Vec<Vec<u8>>,
    sid: u8,
) -> (SessionId, Rx, Vec<Vec<u8>>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let nonce = [sid; 32];
    let (res, replay) = r
        .authenticate(
            [sid; 16],
            &nonce,
            &auth_env(k, chain, nonce, None),
            Some(tx),
            NOW,
        )
        .expect("auth");
    assert!(res.ok);
    ([sid; 16], rx, replay)
}

fn drain(rx: &mut Rx) -> Vec<Outgoing> {
    let mut v = vec![];
    while let Ok(x) = rx.try_recv() {
        v.push(x);
    }
    v
}

#[test]
fn members_authenticate_and_forward() {
    let p = practice();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (b, cb) = member(&p, 11);
    let (sa, mut rxa, _) = connect(&mut r, &a, ca, 1);
    let (sb, mut rxb, _) = connect(&mut r, &b, cb, 2);
    drain(&mut rxa);
    drain(&mut rxb);
    let env = sealed(&a, 1);
    assert!(r.receive(&sa, &env, NOW));
    assert_eq!(drain(&mut rxb), vec![Outgoing::Frame(Frame::Envelope(env))]);
    assert!(drain(&mut rxa).is_empty(), "not echoed to the sender");
    r.disconnect(&sb);
    assert!(matches!(
        drain(&mut rxa).as_slice(),
        [Outgoing::Frame(Frame::Presence(_))]
    ));
}

#[test]
fn bad_auth_is_refused() {
    let p = practice();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (tx, _rx) = mpsc::unbounded_channel();
    // Wrong nonce.
    let e = auth_env(&a, ca.clone(), [1; 32], None);
    assert_eq!(
        r.authenticate([1; 16], &[2; 32], &e, Some(tx.clone()), NOW),
        Err(AuthError(CLOSE_AUTH_FAILED))
    );
    // Not a member (no chain).
    let stranger = DeviceKeys::from_secrets([70; 32], [71; 32]);
    let e = auth_env(&stranger, vec![], [1; 32], None);
    assert_eq!(
        r.authenticate([1; 16], &[1; 32], &e, Some(tx.clone()), NOW),
        Err(AuthError(CLOSE_AUTH_FAILED))
    );
    // Unknown tenant.
    let mut other = Relay::new(Limits::default());
    let e = auth_env(&a, ca, [1; 32], None);
    assert_eq!(
        other.authenticate([1; 16], &[1; 32], &e, Some(tx), NOW),
        Err(AuthError(CLOSE_UNKNOWN_TENANT))
    );
}

#[test]
fn resume_replays_after_cursor() {
    let p = practice();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (b, cb) = member(&p, 11);
    let (sa, _rxa, _) = connect(&mut r, &a, ca, 1);
    for n in 1..=3 {
        r.receive(&sa, &sealed(&a, n), NOW);
    }
    // b connects fresh: gets all three; resuming after the second, only the
    // third.
    let (_, _rxb, replay) = connect(&mut r, &b, cb.clone(), 2);
    let second = first_cursor(NOW) + 1;
    assert_eq!(replay.len(), 3);
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_, replay) = r
        .authenticate(
            [3; 16],
            &[3; 32],
            &auth_env(&b, cb, [3; 32], Some(second)),
            Some(tx),
            NOW,
        )
        .expect("auth");
    assert_eq!(replay, vec![sealed(&a, 3)]);
}

#[test]
fn admin_revocation_closes_and_refuses_the_device() {
    let p = practice();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (_sa, mut rxa, _) = connect(&mut r, &a, ca.clone(), 1);
    let (sadm, _rxadm, _) = connect(&mut r, &p.admin, vec![p.admin_cert.clone()], 2);
    drain(&mut rxa);
    let rev = Body {
        msg_id: MsgId([0xee; 16]),
        created_at: NOW,
        sender: p.admin.device_id(),
        payload: Payload::Revocation(Revocation {
            target_serial: CertSerial([10; 16]),
            target_device: a.device_id(),
            cascade: false,
            reason: "lost".into(),
            new_epoch: 1,
        }),
    };
    let env = open_envelope(PID, &sign_body(&p.admin, &rev)).to_cbor();
    assert!(r.receive(&sadm, &env, NOW));
    let got = drain(&mut rxa);
    assert!(got.contains(&Outgoing::Close(CLOSE_REVOKED)), "{got:?}");
    let (tx, _rx) = mpsc::unbounded_channel();
    assert_eq!(
        r.authenticate(
            [4; 16],
            &[4; 32],
            &auth_env(&a, ca, [4; 32], None),
            Some(tx),
            NOW
        ),
        Err(AuthError(CLOSE_REVOKED))
    );
    let _ = DeviceId::default();
}

#[test]
fn a_member_cannot_revoke() {
    let p = practice();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (b, cb) = member(&p, 11);
    let (sa, _rxa, _) = connect(&mut r, &a, ca, 1);
    let (_sb, mut rxb, _) = connect(&mut r, &b, cb, 2);
    let rev = Body {
        msg_id: MsgId([0xef; 16]),
        created_at: NOW,
        sender: a.device_id(),
        payload: Payload::Revocation(Revocation {
            target_serial: CertSerial([11; 16]),
            target_device: b.device_id(),
            cascade: false,
            reason: String::new(),
            new_epoch: 1,
        }),
    };
    r.receive(
        &sa,
        &open_envelope(PID, &sign_body(&a, &rev)).to_cbor(),
        NOW,
    );
    assert!(!drain(&mut rxb).contains(&Outgoing::Close(CLOSE_REVOKED)));
    assert_eq!(r.session_count(), 2);
}

#[test]
fn rate_limit_drops_excess() {
    let p = practice();
    let mut r = Relay::new(Limits {
        burst_per_conn: 3,
        msgs_per_sec_per_conn: 1,
        ..Limits::default()
    });
    r.add_tenant(PID, p.root.public(), "Test", NOW);
    let (a, ca) = member(&p, 10);
    let (sa, _rx, _) = connect(&mut r, &a, ca, 1);
    for n in 0..10 {
        r.receive(&sa, &sealed(&a, n), NOW);
    }
    assert_eq!(r.metrics.dropped_rate_limited, 7);
}

fn revocation(admin: &DeviceKeys, target: &DeviceKeys, serial: u8, id: u8) -> Vec<u8> {
    let rev = Body {
        msg_id: MsgId([id; 16]),
        created_at: NOW,
        sender: admin.device_id(),
        payload: Payload::Revocation(Revocation {
            target_serial: CertSerial([serial; 16]),
            target_device: target.device_id(),
            cascade: false,
            reason: String::new(),
            new_epoch: 1,
        }),
    };
    open_envelope(PID, &sign_body(admin, &rev)).to_cbor()
}

fn try_auth(
    r: &mut Relay,
    k: &DeviceKeys,
    chain: Vec<Vec<u8>>,
    sid: u8,
    now: u64,
) -> Result<AuthResult, AuthError> {
    let (tx, _rx) = mpsc::unbounded_channel();
    r.authenticate(
        [sid; 16],
        &[sid; 32],
        &auth_env(k, chain, [sid; 32], None),
        Some(tx),
        now,
    )
    .map(|(a, _)| a)
}

#[test]
fn persisted_buffer_and_revocations_survive_a_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("relay.sqlite");
    let p = practice();
    let (a, ca) = member(&p, 10);
    let (b, cb) = member(&p, 11);
    let (c, cc) = member(&p, 12);
    let last = {
        let mut r = relay(&p);
        r.attach_store(Store::open(&path).expect("store"), NOW)
            .expect("attach");
        let (sa, _rxa, _) = connect(&mut r, &a, ca, 1);
        let (sadm, _rx, _) = connect(&mut r, &p.admin, vec![p.admin_cert.clone()], 2);
        r.receive(&sa, &sealed(&a, 1), NOW);
        r.receive(&sa, &sealed(&a, 2), NOW);
        r.receive(&sadm, &revocation(&p.admin, &b, 11, 0xee), NOW);
        try_auth(&mut r, &c, cc.clone(), 3, NOW).expect("c").cursor
    };
    let mut r = Relay::new(Limits::default());
    r.add_tenant(PID, p.root.public(), "Test", NOW + 1);
    r.attach_store(Store::open(&path).expect("store"), NOW + 1)
        .expect("attach");
    let (_, _rx, replay) = connect(&mut r, &c, cc, 4);
    assert_eq!(replay.len(), 3, "two envelopes and the revocation");
    assert_eq!(
        try_auth(&mut r, &b, cb, 5, NOW + 1),
        Err(AuthError(CLOSE_REVOKED))
    );
    assert!(
        try_auth(&mut r, &a, member(&p, 10).1, 6, NOW + 1)
            .expect("a")
            .cursor
            >= last
    );
}

#[test]
fn cursors_increase_across_restarts_without_persistence() {
    let p = practice();
    let (a, ca) = member(&p, 10);
    let mut r = relay(&p);
    let (sa, _rx, _) = connect(&mut r, &a, ca.clone(), 1);
    for n in 0..5 {
        r.receive(&sa, &sealed(&a, n), NOW);
    }
    let before = try_auth(&mut r, &a, ca.clone(), 2, NOW).expect("a").cursor;
    let mut r = Relay::new(Limits::default());
    r.add_tenant(PID, p.root.public(), "Test", NOW + 1);
    let (sa, _rx, _) = connect(&mut r, &a, ca.clone(), 1);
    r.receive(&sa, &sealed(&a, 9), NOW + 1);
    let after = try_auth(&mut r, &a, ca, 2, NOW + 1).expect("a").cursor;
    assert!(after > before, "{after} > {before}");
}

#[test]
fn revocation_by_an_unknown_admin_applies_once_its_cert_arrives() {
    let p = practice();
    let admin2 = DeviceKeys::from_secrets([30; 32], [31; 32]);
    let admin2_cert = issue_cert(&p.root, body(&admin2, Role::Admin, 30)).to_cbor();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (b, cb) = member(&p, 11);
    let (sa, _rxa, _) = connect(&mut r, &a, ca, 1);
    let (_sb, mut rxb, _) = connect(&mut r, &b, cb, 2);
    // a passes on admin2's revocation of b; the relay cannot verify it yet.
    r.receive(&sa, &revocation(&admin2, &b, 11, 0xed), NOW);
    assert!(!drain(&mut rxb).contains(&Outgoing::Close(CLOSE_REVOKED)));
    // admin2 connects: its chain makes the revocation verifiable.
    try_auth(&mut r, &admin2, vec![admin2_cert], 3, NOW).expect("admin2");
    assert!(drain(&mut rxb).contains(&Outgoing::Close(CLOSE_REVOKED)));
    assert_eq!(r.metrics.revocations, 1);
}

#[test]
fn idle_long_poll_sessions_expire() {
    let p = practice();
    let mut r = relay(&p);
    let (a, ca) = member(&p, 10);
    let (b, cb) = member(&p, 11);
    let (_sa, mut rxa, _) = connect(&mut r, &a, ca, 1);
    let lp = [9; 16];
    r.authenticate(lp, &[9; 32], &auth_env(&b, cb, [9; 32], None), None, NOW)
        .expect("lp auth");
    drain(&mut rxa);
    let polled = r.poll(&lp, 0, NOW + LP_IDLE_MS / 2).expect("poll");
    assert_eq!(polled.presence, vec![a.device_id()]);
    r.expire(NOW + LP_IDLE_MS);
    assert_eq!(r.session_count(), 2, "polled recently");
    r.expire(NOW + 2 * LP_IDLE_MS);
    assert_eq!(r.session_count(), 1);
    assert!(matches!(
        drain(&mut rxa).as_slice(),
        [Outgoing::Frame(Frame::Presence(p))] if p.connected.is_empty()
    ));
    assert!(r.poll(&lp, 0, NOW + 2 * LP_IDLE_MS).is_none());
}
