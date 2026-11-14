// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Certificate issuing and chain validation (`spec/04-security.md`,
//! Certificate validation; ADR 0012 for issuer expiry and revocation timing).

use std::collections::{BTreeMap, BTreeSet};

use muster_proto::envelope::context;
use muster_proto::{Cbor, Cert, CertBody, CertSerial, DeviceId, MsgId, PracticeId, Role};

use crate::keys::{DeviceKeys, SigningSecret, verify};
use crate::{CertError, CryptoError};

/// Maximum issuer chain depth.
pub const MAX_CHAIN_DEPTH: usize = 8;
/// Expiry is judged with this much tolerance for clock error.
pub const EXPIRY_TOLERANCE_MS: u64 = 24 * 60 * 60 * 1000;
/// Enroller certs may be valid for at most this long.
pub const ENROLLER_MAX_LIFETIME_MS: u64 = 365 * 24 * 60 * 60 * 1000;

/// Something that can sign certificates.
pub trait CertSigner {
    fn signer_public(&self) -> [u8; 32];
    fn sign_ctx(&self, ctx: &str, data: &[u8]) -> [u8; 64];
}

impl CertSigner for DeviceKeys {
    fn signer_public(&self) -> [u8; 32] {
        self.device_id().0
    }
    fn sign_ctx(&self, ctx: &str, data: &[u8]) -> [u8; 64] {
        self.sign(ctx, data)
    }
}

impl CertSigner for SigningSecret {
    fn signer_public(&self) -> [u8; 32] {
        self.public()
    }
    fn sign_ctx(&self, ctx: &str, data: &[u8]) -> [u8; 64] {
        self.sign(ctx, data)
    }
}

/// Signs a cert body. `body.issuer` is overwritten with the signer's key.
pub fn issue_cert(signer: &impl CertSigner, mut body: CertBody) -> Cert {
    body.issuer = signer.signer_public();
    let bytes = body.to_cbor();
    Cert {
        sig: signer.sign_ctx(context::CERT, &bytes),
        body: bytes,
    }
}

/// Checks a cert's own signature and decodes its body.
pub fn verify_cert_signature(cert: &Cert) -> Result<CertBody, CryptoError> {
    let body = cert.body()?;
    verify(&body.issuer, context::CERT, &cert.body, &cert.sig)?;
    Ok(body)
}

/// A verified revocation (the caller has checked it is signed by a valid
/// admin of this practice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationRecord {
    pub msg_id: MsgId,
    pub target_serial: CertSerial,
    pub target_device: DeviceId,
    pub cascade: bool,
    /// The revocation's `created_at` (ms).
    pub at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RevState {
    cascade: bool,
    /// Earliest non-cascade revocation time.
    earliest: u64,
}

impl RevState {
    fn merge(&mut self, cascade: bool, at: u64) {
        self.cascade |= cascade;
        self.earliest = self.earliest.min(at);
    }
}

#[derive(Debug, Clone)]
struct StoredCert {
    cert: Cert,
    body: CertBody,
}

/// The certificates and revocations a device knows for its practice.
///
/// Certs are stored as soon as their own signature verifies, even if their
/// issuer is not yet known. Validity is evaluated on query, so it does not
/// depend on the order in which certs and revocations arrive (P4).
#[derive(Debug, Clone)]
pub struct TrustStore {
    practice_id: PracticeId,
    root_pub: [u8; 32],
    certs: BTreeMap<CertSerial, StoredCert>,
    by_subject: BTreeMap<DeviceId, BTreeSet<CertSerial>>,
    revoked_serials: BTreeMap<CertSerial, RevState>,
    revoked_devices: BTreeMap<DeviceId, RevState>,
    revocations: BTreeMap<MsgId, RevocationRecord>,
}

impl TrustStore {
    pub fn new(practice_id: PracticeId, root_pub: [u8; 32]) -> Self {
        TrustStore {
            practice_id,
            root_pub,
            certs: BTreeMap::new(),
            by_subject: BTreeMap::new(),
            revoked_serials: BTreeMap::new(),
            revoked_devices: BTreeMap::new(),
            revocations: BTreeMap::new(),
        }
    }

    pub fn practice_id(&self) -> PracticeId {
        self.practice_id
    }

    pub fn root_pub(&self) -> [u8; 32] {
        self.root_pub
    }

    /// Adds a cert after checking its signature and practice id. Returns its
    /// serial, and whether it was new.
    pub fn add_cert(&mut self, cert: Cert) -> Result<(CertSerial, bool), CryptoError> {
        let body = verify_cert_signature(&cert)?;
        if body.practice_id != self.practice_id {
            return Err(CryptoError::Cert(CertError::WrongPractice));
        }
        let serial = body.serial;
        if let Some(existing) = self.certs.get(&serial) {
            // A different cert reusing a serial is suspicious; keep the first.
            return if existing.cert == cert {
                Ok((serial, false))
            } else {
                Err(CryptoError::Cert(CertError::SerialConflict))
            };
        }
        self.by_subject
            .entry(body.subject)
            .or_default()
            .insert(serial);
        self.certs.insert(serial, StoredCert { cert, body });
        Ok((serial, true))
    }

    /// Records a revocation. Returns false if it was already known.
    pub fn add_revocation(&mut self, r: RevocationRecord) -> bool {
        if self.revocations.contains_key(&r.msg_id) {
            return false;
        }
        let fresh = RevState {
            cascade: r.cascade,
            earliest: r.at,
        };
        self.revoked_serials
            .entry(r.target_serial)
            .and_modify(|s| s.merge(r.cascade, r.at))
            .or_insert(fresh);
        self.revoked_devices
            .entry(r.target_device)
            .and_modify(|s| s.merge(r.cascade, r.at))
            .or_insert(fresh);
        self.revocations.insert(r.msg_id, r);
        true
    }

    pub fn revocations(&self) -> impl Iterator<Item = &RevocationRecord> {
        self.revocations.values()
    }

    pub fn certs(&self) -> impl Iterator<Item = (&Cert, &CertBody)> {
        self.certs.values().map(|s| (&s.cert, &s.body))
    }

    pub fn cert(&self, serial: &CertSerial) -> Option<(&Cert, &CertBody)> {
        self.certs.get(serial).map(|s| (&s.cert, &s.body))
    }

    /// Whether a device has been revoked at all (cascade or not).
    pub fn is_device_revoked(&self, device: &DeviceId) -> bool {
        self.revoked_devices.contains_key(device)
    }

    fn rev_state(&self, body: &CertBody) -> Option<RevState> {
        let a = self.revoked_serials.get(&body.serial).copied();
        let b = self.revoked_devices.get(&body.subject).copied();
        match (a, b) {
            (Some(mut x), Some(y)) => {
                x.merge(y.cascade, y.earliest);
                Some(x)
            }
            (x, y) => x.or(y),
        }
    }

    /// Validates the cert with this serial as a leaf at wall time `now`.
    pub fn validate(&self, serial: &CertSerial, now: u64) -> Result<&CertBody, CertError> {
        self.check(serial, now, None, 0)
    }

    /// `as_issuer_of`: when checking an issuer, the `issued_at` of the cert
    /// it issued. Issuers are judged as of that time for expiry and for
    /// non-cascading revocation (ADR 0012); cascading revocation always
    /// invalidates.
    fn check(
        &self,
        serial: &CertSerial,
        now: u64,
        as_issuer_of: Option<u64>,
        depth: usize,
    ) -> Result<&CertBody, CertError> {
        if depth > MAX_CHAIN_DEPTH {
            return Err(CertError::ChainTooDeep);
        }
        let stored = self.certs.get(serial).ok_or(CertError::Unknown)?;
        let body = &stored.body;
        if body.version != muster_proto::cert::CERT_VERSION {
            return Err(CertError::UnsupportedVersion);
        }
        if matches!(body.role, Role::Other(_)) {
            return Err(CertError::UnknownRole);
        }
        // Expiry (rule 4).
        if body.expires_at != 0 {
            let t = as_issuer_of.unwrap_or(now);
            if t > body.expires_at.saturating_add(EXPIRY_TOLERANCE_MS) {
                return Err(CertError::Expired);
            }
        }
        if body.role == Role::Enroller
            && (body.expires_at == 0
                || body.expires_at.saturating_sub(body.issued_at) > ENROLLER_MAX_LIFETIME_MS)
        {
            return Err(CertError::EnrollerLifetime);
        }
        // Revocation (rule 5).
        if let Some(rev) = self.rev_state(body) {
            if rev.cascade {
                return Err(CertError::Revoked);
            }
            match as_issuer_of {
                None => return Err(CertError::Revoked),
                Some(t) if t > rev.earliest => return Err(CertError::Revoked),
                Some(_) => {}
            }
        }
        // Issuer (rule 3).
        if body.issuer == self.root_pub {
            return if body.role == Role::Admin {
                Ok(body)
            } else {
                Err(CertError::IssuerNotAllowed)
            };
        }
        let allowed: &[Role] = match body.role {
            Role::Admin | Role::Enroller => &[Role::Admin],
            Role::Member => &[Role::Admin, Role::Enroller],
            Role::Other(_) => &[],
        };
        let issuer_dev = DeviceId(body.issuer);
        let mut last = CertError::IssuerUnknown;
        for s in self.by_subject.get(&issuer_dev).into_iter().flatten() {
            let Some(ic) = self.certs.get(s) else {
                continue;
            };
            if !allowed.contains(&ic.body.role) {
                last = CertError::IssuerNotAllowed;
                continue;
            }
            match self.check(s, now, Some(body.issued_at), depth + 1) {
                Ok(_) => return Ok(body),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// All currently valid certs of a device.
    pub fn valid_certs(&self, device: &DeviceId, now: u64) -> Vec<&CertBody> {
        self.by_subject
            .get(device)
            .into_iter()
            .flatten()
            .filter_map(|s| self.validate(s, now).ok())
            .collect()
    }

    /// The device's highest valid role (admin over member over enroller), or
    /// `None` if it has no valid cert. Enroller certs are not device roles.
    pub fn role(&self, device: &DeviceId, now: u64) -> Option<Role> {
        let certs = self.valid_certs(device, now);
        if certs.iter().any(|c| c.role == Role::Admin) {
            Some(Role::Admin)
        } else if certs.iter().any(|c| c.role == Role::Member) {
            Some(Role::Member)
        } else {
            None
        }
    }

    pub fn is_member(&self, device: &DeviceId, now: u64) -> bool {
        self.role(device, now).is_some()
    }

    pub fn is_admin(&self, device: &DeviceId, now: u64) -> bool {
        self.role(device, now) == Some(Role::Admin)
    }

    /// A valid device cert for `device` plus its issuer chain up to but not
    /// including the root, leaf first.
    pub fn chain_for(&self, device: &DeviceId, now: u64) -> Option<Vec<Cert>> {
        let leaf = self
            .by_subject
            .get(device)?
            .iter()
            .filter(|s| {
                self.validate(s, now)
                    .is_ok_and(|b| b.role == Role::Admin || b.role == Role::Member)
            })
            .max_by_key(|s| self.certs.get(s).map(|c| c.body.role == Role::Admin))?;
        let mut out = Vec::new();
        let mut cur = *leaf;
        for _ in 0..=MAX_CHAIN_DEPTH {
            let c = self.certs.get(&cur)?;
            out.push(c.cert.clone());
            if c.body.issuer == self.root_pub {
                return Some(out);
            }
            let issued_at = c.body.issued_at;
            cur = *self
                .by_subject
                .get(&DeviceId(c.body.issuer))?
                .iter()
                .find(|s| self.check(s, now, Some(issued_at), 1).is_ok())?;
        }
        None
    }

    /// Serials of certs, sorted, for the state digest.
    pub fn serials(&self) -> impl Iterator<Item = &CertSerial> {
        self.certs.keys()
    }

    /// Ids of revocation messages, sorted, for the state digest.
    pub fn revocation_ids(&self) -> impl Iterator<Item = &MsgId> {
        self.revocations.keys()
    }

    /// Devices with at least one valid membership cert.
    pub fn members(&self, now: u64) -> Vec<DeviceId> {
        self.by_subject
            .keys()
            .filter(|d| self.is_member(d, now))
            .copied()
            .collect()
    }

    /// Prunes certs whose subject was revoked more than `keep_ms` ago, unless
    /// they are still needed as issuers of valid certs.
    pub fn prune_revoked(&mut self, now: u64, keep_ms: u64) {
        let old: Vec<CertSerial> = self
            .certs
            .iter()
            .filter(|(_, c)| {
                self.rev_state(&c.body)
                    .is_some_and(|r| now.saturating_sub(r.earliest) > keep_ms)
            })
            .map(|(s, _)| *s)
            .collect();
        for s in old {
            let Some(c) = self.certs.get(&s) else {
                continue;
            };
            let subject = c.body.subject;
            let issues_valid = self
                .certs
                .values()
                .any(|o| o.body.issuer == subject.0 && self.validate(&o.body.serial, now).is_ok());
            if issues_valid {
                continue;
            }
            self.certs.remove(&s);
            if let Some(set) = self.by_subject.get_mut(&subject) {
                set.remove(&s);
                if set.is_empty() {
                    self.by_subject.remove(&subject);
                }
            }
        }
    }
}
