// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The outermost structure on every transport, and the signed message inside.

use crate::proto::cbor::{Cbor, Reader, Writer, req};
use crate::proto::error::DecodeError;
use crate::proto::ids::{PracticeId, PracticeTag};
use crate::proto::limits;

/// Envelope kind codes.
pub const KIND_SEALED: u64 = 0;
pub const KIND_OPEN: u64 = 1;

/// A group-encrypted envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedEnvelope {
    pub tag: PracticeTag,
    pub epoch: u64,
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

impl SealedEnvelope {
    /// The AEAD associated data: `CBOR({0: version, 1: kind, 2: tag, 3: epoch})`.
    pub fn aad(tag: &PracticeTag, epoch: u64) -> Vec<u8> {
        let mut w = Writer::new();
        w.map(4)
            .u64(0)
            .u64(limits::ENVELOPE_VERSION)
            .u64(1)
            .u64(KIND_SEALED)
            .u64(2)
            .bytes(&tag.0)
            .u64(3)
            .u64(epoch);
        w.into_bytes()
    }
}

/// A signed but not group-encrypted envelope (join, approval, key share,
/// revocation, relay auth).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEnvelope {
    pub practice_id: PracticeId,
    /// CBOR bytes of a [`SignedMessage`].
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Envelope {
    Sealed(SealedEnvelope),
    Open(OpenEnvelope),
}

impl Envelope {
    /// Cheap header peek: returns the sealed tag without decoding the
    /// payload copy. Used to drop other practices' traffic early
    /// (FR-NET-004).
    pub fn peek_sealed_tag(bytes: &[u8]) -> Option<PracticeTag> {
        match Envelope::from_cbor(bytes) {
            Ok(Envelope::Sealed(s)) => Some(s.tag),
            _ => None,
        }
    }
}

impl Cbor for Envelope {
    fn encode(&self, w: &mut Writer) {
        match self {
            Envelope::Sealed(s) => {
                w.map(6)
                    .u64(0)
                    .u64(limits::ENVELOPE_VERSION)
                    .u64(1)
                    .u64(KIND_SEALED)
                    .u64(2)
                    .bytes(&s.tag.0)
                    .u64(3)
                    .u64(s.epoch)
                    .u64(4)
                    .bytes(&s.nonce)
                    .u64(5)
                    .bytes(&s.ciphertext);
            }
            Envelope::Open(o) => {
                w.map(4)
                    .u64(0)
                    .u64(limits::ENVELOPE_VERSION)
                    .u64(1)
                    .u64(KIND_OPEN)
                    .u64(2)
                    .bytes(&o.practice_id.0)
                    .u64(5)
                    .bytes(&o.payload);
            }
        }
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut version = None;
        let mut kind = None;
        let mut tag: Option<Vec<u8>> = None;
        let mut epoch = None;
        let mut nonce = None;
        let mut payload: Option<Vec<u8>> = None;
        r.map(|k, r| {
            match k {
                0 => version = Some(r.u64()?),
                1 => kind = Some(r.u64()?),
                2 => tag = Some(r.bytes(16)?.to_vec()),
                3 => epoch = Some(r.u64()?),
                4 => nonce = Some(r.fixed::<24>()?),
                5 => payload = Some(r.bytes(limits::TCP_FRAME_MAX)?.to_vec()),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let version = req(version, 0)?;
        if version != limits::ENVELOPE_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        let tag = req(tag, 2)?;
        let payload = req(payload, 5)?;
        match req(kind, 1)? {
            KIND_SEALED => Ok(Envelope::Sealed(SealedEnvelope {
                tag: PracticeTag(
                    <[u8; 8]>::try_from(tag.as_slice()).map_err(|_| DecodeError::WrongLength)?,
                ),
                epoch: req(epoch, 3)?,
                nonce: req(nonce, 4)?,
                ciphertext: payload,
            })),
            KIND_OPEN => Ok(Envelope::Open(OpenEnvelope {
                practice_id: PracticeId(
                    <[u8; 16]>::try_from(tag.as_slice()).map_err(|_| DecodeError::WrongLength)?,
                ),
                payload,
            })),
            _ => Err(DecodeError::InvalidValue),
        }
    }
}

/// `{0: body bstr, 1: sig bstr(64)}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedMessage {
    /// CBOR bytes of a [`crate::proto::Body`].
    pub body: Vec<u8>,
    pub sig: [u8; 64],
}

impl Cbor for SignedMessage {
    fn encode(&self, w: &mut Writer) {
        w.map(2).u64(0).bytes(&self.body).u64(1).bytes(&self.sig);
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut body = None;
        let mut sig = None;
        r.map(|k, r| {
            match k {
                0 => body = Some(r.bytes(limits::TCP_FRAME_MAX)?.to_vec()),
                1 => sig = Some(r.fixed::<64>()?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(SignedMessage {
            body: req(body, 0)?,
            sig: req(sig, 1)?,
        })
    }
}

/// Signature context strings (`spec/03-protocol.md`, Conventions). The
/// signed bytes are `context || 0x00 || data`.
pub mod context {
    pub const SEALED: &str = "panic-button/v1/sealed";
    pub const OPEN: &str = "panic-button/v1/open";
    pub const CERT: &str = "panic-button/v1/cert";
    pub const RELAY_AUTH: &str = "panic-button/v1/relay-auth";

    /// Builds `context || 0x00 || data`.
    pub fn signing_input(context: &str, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(context.len() + 1 + data.len());
        v.extend_from_slice(context.as_bytes());
        v.push(0);
        v.extend_from_slice(data);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sealed() -> Envelope {
        Envelope::Sealed(SealedEnvelope {
            tag: PracticeTag([1; 8]),
            epoch: 3,
            nonce: [2; 24],
            ciphertext: vec![9, 9, 9],
        })
    }

    #[test]
    fn round_trips() {
        for e in [
            sealed(),
            Envelope::Open(OpenEnvelope {
                practice_id: PracticeId([7; 16]),
                payload: vec![1, 2],
            }),
        ] {
            assert_eq!(Envelope::from_cbor(&e.to_cbor()), Ok(e));
        }
    }

    #[test]
    fn known_encoding() {
        // Hand-assembled from the spec: {0:1, 1:0, 2:h'01..', 3:3, 4:h'02..', 5:h'090909'}
        let expected = format!(
            "a6000101000248{}0303045818{}0543090909",
            "01".repeat(8),
            "02".repeat(24)
        );
        assert_eq!(hex::encode(sealed().to_cbor()), expected);
    }

    #[test]
    fn rejects_other_versions() {
        let mut b = sealed().to_cbor();
        b[2] = 0x02; // version 2
        assert_eq!(
            Envelope::from_cbor(&b),
            Err(DecodeError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn rejects_wrong_tag_length() {
        let mut w = Writer::new();
        w.map(6)
            .u64(0)
            .u64(1)
            .u64(1)
            .u64(0)
            .u64(2)
            .bytes(&[0; 7])
            .u64(3)
            .u64(0)
            .u64(4)
            .bytes(&[0; 24])
            .u64(5)
            .bytes(&[]);
        assert_eq!(
            Envelope::from_cbor(&w.into_bytes()),
            Err(DecodeError::WrongLength)
        );
    }

    #[test]
    fn ignores_unknown_keys() {
        let mut w = Writer::new();
        w.map(5)
            .u64(0)
            .u64(1)
            .u64(1)
            .u64(1)
            .u64(2)
            .bytes(&[5; 16])
            .u64(5)
            .bytes(&[1])
            .u64(40)
            .text("future");
        assert!(matches!(
            Envelope::from_cbor(&w.into_bytes()),
            Ok(Envelope::Open(_))
        ));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut b = sealed().to_cbor();
        b.push(0);
        assert_eq!(Envelope::from_cbor(&b), Err(DecodeError::Trailing));
    }

    #[test]
    fn signing_input_has_separator() {
        assert_eq!(context::signing_input("ab", b"c"), b"ab\0c".to_vec());
    }
}
