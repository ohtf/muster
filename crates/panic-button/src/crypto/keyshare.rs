// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! KeyShare: a group key sealed to one recipient device
//! (`spec/03-protocol.md`, 0x42 KeyShare).
//!
//! `k = HKDF-SHA256(ikm = X25519(eph, recipient_kx), salt = practice_id,
//! info = "panic-button/v1/keyshare" || recipient || epoch_be64)`;
//! `ct = XChaCha20-Poly1305(k, nonce, aad = recipient || epoch_be64,
//! plaintext = group_key)`.

use crate::proto::{DeviceId, KeyShare, PracticeId};
use chacha20poly1305::aead::{Aead, Payload as AeadPayload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey as KxPublic, StaticSecret};
use zeroize::Zeroize;

use crate::crypto::CryptoError;
use crate::crypto::keys::{DeviceKeys, GroupKey};

fn derive(
    shared: &[u8; 32],
    practice_id: &PracticeId,
    recipient: &DeviceId,
    epoch: u64,
) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(&practice_id.0), shared);
    let mut info = Vec::with_capacity(18 + 32 + 8);
    info.extend_from_slice(b"panic-button/v1/keyshare");
    info.extend_from_slice(&recipient.0);
    info.extend_from_slice(&epoch.to_be_bytes());
    let mut okm = [0u8; 32];
    // 32 bytes is always a valid HKDF-SHA256 output length.
    let _ = hk.expand(&info, &mut okm);
    okm
}

fn aad(recipient: &DeviceId, epoch: u64) -> Vec<u8> {
    let mut a = Vec::with_capacity(40);
    a.extend_from_slice(&recipient.0);
    a.extend_from_slice(&epoch.to_be_bytes());
    a
}

/// Builds a KeyShare. `eph_secret` and `nonce` must be fresh random bytes.
pub fn make_keyshare(
    eph_secret: [u8; 32],
    nonce: [u8; 24],
    practice_id: &PracticeId,
    recipient: DeviceId,
    recipient_kx_pub: &[u8; 32],
    epoch: u64,
    group_key: &GroupKey,
) -> Result<KeyShare, CryptoError> {
    let mut es = eph_secret;
    let eph = StaticSecret::from(es);
    es.zeroize();
    let eph_pub = KxPublic::from(&eph).to_bytes();
    let shared = eph.diffie_hellman(&KxPublic::from(*recipient_kx_pub));
    if !shared.was_contributory() {
        return Err(CryptoError::BadPublicKey);
    }
    let mut k = derive(shared.as_bytes(), practice_id, &recipient, epoch);
    let cipher = XChaCha20Poly1305::new((&k).into());
    k.zeroize();
    let ct = cipher
        .encrypt(
            &XNonce::from(nonce),
            AeadPayload {
                msg: group_key.as_bytes(),
                aad: &aad(&recipient, epoch),
            },
        )
        .map_err(|_| CryptoError::DecryptFailed)?;
    Ok(KeyShare {
        recipient,
        epoch,
        eph_pub,
        nonce,
        ct,
    })
}

/// Opens a KeyShare addressed to `me`.
pub fn open_keyshare(
    me: &DeviceKeys,
    practice_id: &PracticeId,
    ks: &KeyShare,
) -> Result<GroupKey, CryptoError> {
    if ks.recipient != me.device_id() {
        return Err(CryptoError::NotForMe);
    }
    let shared = me.kx_secret().diffie_hellman(&KxPublic::from(ks.eph_pub));
    if !shared.was_contributory() {
        return Err(CryptoError::BadPublicKey);
    }
    let mut k = derive(shared.as_bytes(), practice_id, &ks.recipient, ks.epoch);
    let cipher = XChaCha20Poly1305::new((&k).into());
    k.zeroize();
    let mut pt = cipher
        .decrypt(
            &XNonce::from(ks.nonce),
            AeadPayload {
                msg: &ks.ct,
                aad: &aad(&ks.recipient, ks.epoch),
            },
        )
        .map_err(|_| CryptoError::DecryptFailed)?;
    let out = <[u8; 32]>::try_from(pt.as_slice()).map_err(|_| CryptoError::DecryptFailed);
    pt.zeroize();
    Ok(GroupKey::from_bytes(out?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_binding() {
        let alice = DeviceKeys::from_secrets([1; 32], [2; 32]);
        let bob = DeviceKeys::from_secrets([3; 32], [4; 32]);
        let pid = PracticeId([5; 16]);
        let gk = GroupKey::from_bytes([6; 32]);
        let ks = make_keyshare(
            [7; 32],
            [8; 24],
            &pid,
            bob.device_id(),
            &bob.kx_pub(),
            2,
            &gk,
        )
        .expect("makes");
        assert_eq!(open_keyshare(&bob, &pid, &ks).expect("opens"), gk);
        // Not for Alice.
        assert_eq!(open_keyshare(&alice, &pid, &ks), Err(CryptoError::NotForMe));
        // Wrong practice id (HKDF salt) fails.
        assert!(open_keyshare(&bob, &PracticeId([0; 16]), &ks).is_err());
        // The epoch is bound.
        let mut ks2 = ks.clone();
        ks2.epoch = 3;
        assert!(open_keyshare(&bob, &pid, &ks2).is_err());
    }

    #[test]
    fn rejects_low_order_point() {
        let bob = DeviceKeys::from_secrets([3; 32], [4; 32]);
        let r = make_keyshare(
            [7; 32],
            [8; 24],
            &PracticeId([5; 16]),
            bob.device_id(),
            &[0; 32],
            1,
            &GroupKey::from_bytes([1; 32]),
        );
        assert_eq!(r, Err(CryptoError::BadPublicKey));
    }
}
