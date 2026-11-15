// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Certificate wire format (`spec/03-protocol.md`, Certificates). Validation
//! lives in `panic-button-crypto`.

use crate::cbor::{Cbor, MapBuilder, Reader, Writer, req};
use crate::error::DecodeError;
use crate::ids::{CertSerial, DeviceId, PracticeId, TokenId};
use crate::message::open_enum;

open_enum!(
    /// Certificate role. Unknown roles carry no authority.
    Role {
        Member = 0,
        Admin = 1,
        Enroller = 2,
    }
);

pub const CERT_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertBody {
    pub version: u64,
    pub practice_id: PracticeId,
    pub serial: CertSerial,
    /// The subject's Ed25519 signing public key (its device id).
    pub subject: DeviceId,
    /// The subject's X25519 public key.
    pub subject_kx: [u8; 32],
    pub role: Role,
    pub label: String,
    pub issued_at: u64,
    /// 0 means no expiry (not allowed for enroller certs).
    pub expires_at: u64,
    /// The root public key, or the issuing device's signing key.
    pub issuer: [u8; 32],
    /// Set when the cert was self-issued with an enrolment token.
    pub token_id: Option<TokenId>,
}

impl Cbor for CertBody {
    fn encode(&self, w: &mut Writer) {
        let mut m = MapBuilder::new();
        m.entry(0, |w| {
            w.u64(self.version);
        });
        m.entry(1, |w| self.practice_id.encode(w));
        m.entry(2, |w| self.serial.encode(w));
        m.entry(3, |w| self.subject.encode(w));
        m.entry(4, |w| {
            w.bytes(&self.subject_kx);
        });
        m.entry(5, |w| {
            w.u64(self.role.code());
        });
        m.entry(6, |w| {
            w.text(&self.label);
        });
        m.entry(7, |w| {
            w.u64(self.issued_at);
        });
        m.entry(8, |w| {
            w.u64(self.expires_at);
        });
        m.entry(9, |w| {
            w.bytes(&self.issuer);
        });
        if let Some(t) = &self.token_id {
            m.entry(10, |w| t.encode(w));
        }
        m.write(w);
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut version = None;
        let mut practice_id = None;
        let mut serial = None;
        let mut subject = None;
        let mut subject_kx = None;
        let mut role = None;
        let mut label = None;
        let mut issued_at = None;
        let mut expires_at = None;
        let mut issuer = None;
        let mut token_id = None;
        r.map(|k, r| {
            match k {
                0 => version = Some(r.u64()?),
                1 => practice_id = Some(PracticeId::decode(r)?),
                2 => serial = Some(CertSerial::decode(r)?),
                3 => subject = Some(DeviceId::decode(r)?),
                4 => subject_kx = Some(r.fixed::<32>()?),
                5 => role = Some(Role::from_code(r.u64()?)),
                6 => label = Some(r.text()?),
                7 => issued_at = Some(r.u64()?),
                8 => expires_at = Some(r.u64()?),
                9 => issuer = Some(r.fixed::<32>()?),
                10 => token_id = Some(TokenId::decode(r)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(CertBody {
            version: req(version, 0)?,
            practice_id: req(practice_id, 1)?,
            serial: req(serial, 2)?,
            subject: req(subject, 3)?,
            subject_kx: req(subject_kx, 4)?,
            role: req(role, 5)?,
            label: req(label, 6)?,
            issued_at: req(issued_at, 7)?,
            expires_at: req(expires_at, 8)?,
            issuer: req(issuer, 9)?,
            token_id,
        })
    }
}

/// `Cert = {0: body bstr, 1: sig bstr(64)}`. The signature is over
/// `"panic-button/v1/cert" || 0x00 || body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cert {
    pub body: Vec<u8>,
    pub sig: [u8; 64],
}

impl Cert {
    /// Decodes the body. Validation is a separate step.
    pub fn body(&self) -> Result<CertBody, DecodeError> {
        CertBody::from_cbor(&self.body)
    }
}

impl Cbor for Cert {
    fn encode(&self, w: &mut Writer) {
        w.map(2).u64(0).bytes(&self.body).u64(1).bytes(&self.sig);
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let (mut body, mut sig) = (None, None);
        r.map(|k, r| {
            match k {
                0 => body = Some(r.bytes(4096)?.to_vec()),
                1 => sig = Some(r.fixed::<64>()?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Cert {
            body: req(body, 0)?,
            sig: req(sig, 1)?,
        })
    }
}
