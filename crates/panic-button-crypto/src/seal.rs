// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Signing messages and sealing envelopes (`spec/03-protocol.md`, Envelope).

use chacha20poly1305::aead::{Aead, Payload as AeadPayload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use panic_button_proto::envelope::context;
use panic_button_proto::{
    Body, Cbor, Envelope, OpenEnvelope, PracticeId, SealedEnvelope, SignedMessage,
};

use crate::CryptoError;
use crate::keys::{DeviceKeys, GroupKey, verify};

/// Signs a body. Open message types use the `panic-button/v1/open` context and
/// sealed types `panic-button/v1/sealed`, so a signature cannot be replayed from
/// one envelope kind into the other.
pub fn sign_body(keys: &DeviceKeys, body: &Body) -> SignedMessage {
    let bytes = body.to_cbor();
    let ctx = if body.msg_type().is_open() {
        context::OPEN
    } else {
        context::SEALED
    };
    SignedMessage {
        sig: keys.sign(ctx, &bytes),
        body: bytes,
    }
}

/// Decodes and verifies a signed message: the signature must be by
/// `body.sender`, under the context matching the envelope kind it arrived
/// in. Certificate checks are the caller's job.
pub fn verify_signed(signed: &SignedMessage, arrived_open: bool) -> Result<Body, CryptoError> {
    let body = Body::from_cbor(&signed.body)?;
    if body.msg_type().is_open() != arrived_open {
        return Err(CryptoError::WrongEnvelopeKind);
    }
    let ctx = if arrived_open {
        context::OPEN
    } else {
        context::SEALED
    };
    verify(&body.sender.0, ctx, &signed.body, &signed.sig)?;
    Ok(body)
}

/// Seals signed-message bytes under the group key for `epoch`.
pub fn seal(key: &GroupKey, epoch: u64, nonce: [u8; 24], signed: &SignedMessage) -> Envelope {
    let tag = key.tag();
    let aad = SealedEnvelope::aad(&tag, epoch);
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let plaintext = signed.to_cbor();
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            AeadPayload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        // XChaCha20-Poly1305 only fails for messages over 256 GiB.
        .unwrap_or_default();
    Envelope::Sealed(SealedEnvelope {
        tag,
        epoch,
        nonce,
        ciphertext,
    })
}

/// Opens a sealed envelope and decodes the signed message inside. The tag
/// and epoch are authenticated as associated data.
pub fn open(key: &GroupKey, env: &SealedEnvelope) -> Result<SignedMessage, CryptoError> {
    let aad = SealedEnvelope::aad(&env.tag, env.epoch);
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let plaintext = cipher
        .decrypt(
            &XNonce::from(env.nonce),
            AeadPayload {
                msg: &env.ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::DecryptFailed)?;
    Ok(SignedMessage::from_cbor(&plaintext)?)
}

/// Wraps a signed open-type message in an open envelope.
pub fn open_envelope(practice_id: PracticeId, signed: &SignedMessage) -> Envelope {
    Envelope::Open(OpenEnvelope {
        practice_id,
        payload: signed.to_cbor(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use panic_button_proto::{
        AlarmKind, AlarmRaised, DeviceId, MsgId, Payload, Revocation, Source,
    };

    fn keys() -> DeviceKeys {
        DeviceKeys::from_secrets([7; 32], [8; 32])
    }

    fn alarm(sender: DeviceId) -> Body {
        Body {
            msg_id: MsgId([1; 16]),
            created_at: 1000,
            sender,
            payload: Payload::AlarmRaised(AlarmRaised {
                alarm_id: MsgId([1; 16]),
                kind: AlarmKind::Help,
                drill: false,
                room: "Room 1".into(),
                user_display: "A".into(),
                device_label: "PC".into(),
                source: Source::Button,
            }),
        }
    }

    #[test]
    fn seal_open_round_trip() {
        let k = keys();
        let body = alarm(k.device_id());
        let signed = sign_body(&k, &body);
        let gk = GroupKey::from_bytes([9; 32]);
        let Envelope::Sealed(env) = seal(&gk, 4, [3; 24], &signed) else {
            panic!("sealed")
        };
        assert_eq!(env.tag, gk.tag());
        let got = open(&gk, &env).expect("opens");
        assert_eq!(verify_signed(&got, false).expect("verifies"), body);
    }

    #[test]
    fn wrong_key_or_tampering_fails() {
        let k = keys();
        let signed = sign_body(&k, &alarm(k.device_id()));
        let gk = GroupKey::from_bytes([9; 32]);
        let Envelope::Sealed(env) = seal(&gk, 4, [3; 24], &signed) else {
            panic!("sealed")
        };
        assert!(open(&GroupKey::from_bytes([10; 32]), &env).is_err());
        let mut e2 = env.clone();
        e2.epoch = 5; // AAD covers the epoch
        assert!(open(&gk, &e2).is_err());
        let mut e3 = env.clone();
        e3.ciphertext[0] ^= 1;
        assert!(open(&gk, &e3).is_err());
    }

    #[test]
    fn forged_sender_is_rejected() {
        let k = keys();
        let mut body = alarm(k.device_id());
        let mut signed = sign_body(&k, &body);
        // Claim to be someone else.
        body.sender = DeviceId(DeviceKeys::from_secrets([1; 32], [1; 32]).device_id().0);
        signed.body = body.to_cbor();
        assert_eq!(
            verify_signed(&signed, false),
            Err(CryptoError::BadSignature)
        );
    }

    #[test]
    fn envelope_kind_must_match_type() {
        let k = keys();
        let signed = sign_body(&k, &alarm(k.device_id()));
        assert_eq!(
            verify_signed(&signed, true),
            Err(CryptoError::WrongEnvelopeKind)
        );
        let rev = Body {
            msg_id: MsgId([2; 16]),
            created_at: 1,
            sender: k.device_id(),
            payload: Payload::Revocation(Revocation {
                target_serial: panic_button_proto::CertSerial([0; 16]),
                target_device: DeviceId([0; 32]),
                cascade: false,
                reason: String::new(),
                new_epoch: 1,
            }),
        };
        let signed = sign_body(&k, &rev);
        assert!(verify_signed(&signed, true).is_ok());
        assert_eq!(
            verify_signed(&signed, false),
            Err(CryptoError::WrongEnvelopeKind)
        );
    }
}
