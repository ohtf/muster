// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The practice recovery kit (`spec/04-security.md`, flows A and D).
//!
//! Plaintext: `CBOR {0: practice_id, 1: practice_name, 2: root_secret_key,
//! 3: created_at}`. File: `CBOR {0: version, 1: salt bstr(16),
//! 2: nonce bstr(24), 3: ciphertext, 4: m_cost_kib, 5: t_cost, 6: p_cost}`,
//! encrypted with XChaCha20-Poly1305 under an Argon2id key. The KDF
//! parameters are stored so they can be raised later; decoding refuses
//! parameters below the v1 minimum or above a memory ceiling.

use std::fmt;

use crate::proto::cbor::{Cbor, Reader, Writer, req};
use crate::proto::{DecodeError, PracticeId};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, Payload as AeadPayload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

use crate::crypto::CryptoError;
use crate::crypto::keys::SigningSecret;

pub const KIT_VERSION: u64 = 1;
/// File extension for saved kits.
pub const KIT_EXTENSION: &str = "panic-button-recovery";
/// Minimum passphrase length in characters.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// v1 parameters: m = 256 MiB, t = 3, p = 1.
    pub const V1: KdfParams = KdfParams {
        m_cost_kib: 256 * 1024,
        t_cost: 3,
        p_cost: 1,
    };
    /// Refuse to allocate more than 2 GiB when opening a kit.
    pub const MAX_M_COST_KIB: u32 = 2 * 1024 * 1024;

    /// Cheap parameters for tests only. Kits made with these cannot be
    /// opened by [`open_kit`], which enforces the v1 minimum.
    #[doc(hidden)]
    pub const INSECURE_TEST: KdfParams = KdfParams {
        m_cost_kib: 64,
        t_cost: 1,
        p_cost: 1,
    };
}

fn derive_key(
    pass: &str,
    salt: &[u8; 16],
    p: KdfParams,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let params =
        Params::new(p.m_cost_kib, p.t_cost, p.p_cost, Some(32)).map_err(|_| CryptoError::Kdf)?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    a.hash_password_into(pass.as_bytes(), salt, out.as_mut())
        .map_err(|_| CryptoError::Kdf)?;
    Ok(out)
}

/// Checks the passphrase policy (minimum length). Strength metering is a UI
/// concern.
pub fn check_passphrase(pass: &str) -> Result<(), CryptoError> {
    if pass.chars().count() < MIN_PASSPHRASE_CHARS {
        Err(CryptoError::WeakPassphrase)
    } else {
        Ok(())
    }
}

/// The decrypted kit contents. The root secret is zeroized on drop.
pub struct RecoveryContents {
    pub practice_id: PracticeId,
    pub practice_name: String,
    pub root: SigningSecret,
    pub created_at: u64,
}

impl fmt::Debug for RecoveryContents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecoveryContents")
            .field("practice_id", &self.practice_id)
            .field("practice_name", &self.practice_name)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// An encrypted kit as saved to a file or QR code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryKit {
    pub salt: [u8; 16],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
    pub params: KdfParams,
}

impl Cbor for RecoveryKit {
    fn encode(&self, w: &mut Writer) {
        w.map(7)
            .u64(0)
            .u64(KIT_VERSION)
            .u64(1)
            .bytes(&self.salt)
            .u64(2)
            .bytes(&self.nonce)
            .u64(3)
            .bytes(&self.ciphertext)
            .u64(4)
            .u64(u64::from(self.params.m_cost_kib))
            .u64(5)
            .u64(u64::from(self.params.t_cost))
            .u64(6)
            .u64(u64::from(self.params.p_cost));
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let (mut v, mut salt, mut nonce, mut ct, mut m, mut t, mut p) =
            (None, None, None, None, None, None, None);
        let u32v =
            |r: &mut Reader<'_>| u32::try_from(r.u64()?).map_err(|_| DecodeError::InvalidValue);
        r.map(|k, r| {
            match k {
                0 => v = Some(r.u64()?),
                1 => salt = Some(r.fixed::<16>()?),
                2 => nonce = Some(r.fixed::<24>()?),
                3 => ct = Some(r.bytes(1024)?.to_vec()),
                4 => m = Some(u32v(r)?),
                5 => t = Some(u32v(r)?),
                6 => p = Some(u32v(r)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        if req(v, 0)? != KIT_VERSION {
            return Err(DecodeError::InvalidValue);
        }
        Ok(RecoveryKit {
            salt: req(salt, 1)?,
            nonce: req(nonce, 2)?,
            ciphertext: req(ct, 3)?,
            params: KdfParams {
                m_cost_kib: req(m, 4)?,
                t_cost: req(t, 5)?,
                p_cost: req(p, 6)?,
            },
        })
    }
}

fn plaintext(c: &RecoveryContents) -> Zeroizing<Vec<u8>> {
    let mut secret = c.root.to_secret();
    let mut w = Writer::new();
    w.map(4)
        .u64(0)
        .bytes(&c.practice_id.0)
        .u64(1)
        .text(&c.practice_name)
        .u64(2)
        .bytes(&secret)
        .u64(3)
        .u64(c.created_at);
    secret.zeroize();
    Zeroizing::new(w.into_bytes())
}

/// Encrypts a recovery kit. `salt` and `nonce` must be fresh random bytes.
pub fn seal_kit(
    contents: &RecoveryContents,
    pass: &str,
    salt: [u8; 16],
    nonce: [u8; 24],
    params: KdfParams,
) -> Result<RecoveryKit, CryptoError> {
    check_passphrase(pass)?;
    let key = derive_key(pass, &salt, params)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let pt = plaintext(contents);
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            AeadPayload {
                msg: &pt,
                aad: b"panic-button/v1/recovery",
            },
        )
        .map_err(|_| CryptoError::Kdf)?;
    Ok(RecoveryKit {
        salt,
        nonce,
        ciphertext,
        params,
    })
}

fn open_with(kit: &RecoveryKit, pass: &str) -> Result<RecoveryContents, CryptoError> {
    if kit.params.m_cost_kib > KdfParams::MAX_M_COST_KIB {
        return Err(CryptoError::Kdf);
    }
    let key = derive_key(pass, &kit.salt, kit.params)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let pt = Zeroizing::new(
        cipher
            .decrypt(
                &XNonce::from(kit.nonce),
                AeadPayload {
                    msg: &kit.ciphertext,
                    aad: b"panic-button/v1/recovery",
                },
            )
            .map_err(|_| CryptoError::WrongPassphrase)?,
    );
    let mut r = Reader::new(&pt);
    let (mut pid, mut name, mut secret, mut at) = (None, None, None, None);
    r.map(|k, r| {
        match k {
            0 => pid = Some(PracticeId::decode(r)?),
            1 => name = Some(r.text()?),
            2 => secret = Some(Zeroizing::new(r.fixed::<32>()?)),
            3 => at = Some(r.u64()?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    r.finish()?;
    let secret = req(secret, 2)?;
    Ok(RecoveryContents {
        practice_id: req(pid, 0)?,
        practice_name: req(name, 1)?,
        root: SigningSecret::from_secret(*secret),
        created_at: req(at, 3)?,
    })
}

/// Decrypts a kit. Refuses KDF parameters weaker than v1, so a tampered
/// file cannot downgrade the work factor a guesser faces.
pub fn open_kit(kit: &RecoveryKit, pass: &str) -> Result<RecoveryContents, CryptoError> {
    let p = kit.params;
    if p.m_cost_kib < KdfParams::V1.m_cost_kib || p.t_cost < KdfParams::V1.t_cost {
        return Err(CryptoError::Kdf);
    }
    open_with(kit, pass)
}

/// Opens a kit made with [`KdfParams::INSECURE_TEST`]. Tests only.
#[doc(hidden)]
pub fn open_kit_insecure_for_tests(
    kit: &RecoveryKit,
    pass: &str,
) -> Result<RecoveryContents, CryptoError> {
    open_with(kit, pass)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents() -> RecoveryContents {
        RecoveryContents {
            practice_id: PracticeId([1; 16]),
            practice_name: "Riverside Surgery".into(),
            root: SigningSecret::from_secret([2; 32]),
            created_at: 3,
        }
    }

    const PASS: &str = "correct horse battery";

    #[test]
    fn round_trip() {
        let kit = seal_kit(
            &contents(),
            PASS,
            [4; 16],
            [5; 24],
            KdfParams::INSECURE_TEST,
        )
        .expect("seals");
        let bytes = kit.to_cbor();
        let back = RecoveryKit::from_cbor(&bytes).expect("decodes");
        let c = open_kit_insecure_for_tests(&back, PASS).expect("opens");
        assert_eq!(c.practice_id, PracticeId([1; 16]));
        assert_eq!(
            c.root.public(),
            SigningSecret::from_secret([2; 32]).public()
        );
    }

    #[test]
    fn wrong_passphrase_fails() {
        let kit = seal_kit(
            &contents(),
            PASS,
            [4; 16],
            [5; 24],
            KdfParams::INSECURE_TEST,
        )
        .expect("seals");
        assert!(matches!(
            open_kit_insecure_for_tests(&kit, "correct horse battery!"),
            Err(CryptoError::WrongPassphrase)
        ));
    }

    #[test]
    fn weak_params_refused_by_open_kit() {
        let kit = seal_kit(
            &contents(),
            PASS,
            [4; 16],
            [5; 24],
            KdfParams::INSECURE_TEST,
        )
        .expect("seals");
        assert!(matches!(open_kit(&kit, PASS), Err(CryptoError::Kdf)));
    }

    #[test]
    fn short_passphrase_refused() {
        assert!(matches!(
            seal_kit(
                &contents(),
                "short",
                [4; 16],
                [5; 24],
                KdfParams::INSECURE_TEST
            ),
            Err(CryptoError::WeakPassphrase)
        ));
        // Counted in characters, not bytes.
        assert!(check_passphrase("ééééééééééé").is_err());
        assert!(check_passphrase("éééééééééééé").is_ok());
    }
}
