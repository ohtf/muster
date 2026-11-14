// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Muster cryptography: device keys, certificates, sealing, key shares, recovery kits and tokens (no I/O).
//!
//! Primitives come from audited RustCrypto and dalek crates (ADR 0005):
//! Ed25519, X25519, XChaCha20-Poly1305, BLAKE3, HKDF-SHA256 and Argon2id.
//! Nothing here reads clocks or draws randomness; callers supply both.

#![forbid(unsafe_code)]

pub mod cert;
pub mod keys;
pub mod keyshare;
pub mod recovery;
pub mod seal;
pub mod token;

pub use cert::{RevocationRecord, TrustStore, issue_cert};
pub use keys::{DeviceKeys, GroupKey, SigningSecret, join_code, verify};
pub use keyshare::{make_keyshare, open_keyshare};
pub use seal::{open, open_envelope, seal, sign_body, verify_signed};

use muster_proto::DecodeError;

/// Why a certificate is not valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CertError {
    #[error("certificate not known")]
    Unknown,
    #[error("certificate is for another practice")]
    WrongPractice,
    #[error("unsupported certificate version")]
    UnsupportedVersion,
    #[error("unknown certificate role")]
    UnknownRole,
    #[error("certificate expired")]
    Expired,
    #[error("enroller certificate lifetime missing or over 365 days")]
    EnrollerLifetime,
    #[error("certificate revoked")]
    Revoked,
    #[error("issuer certificate not known")]
    IssuerUnknown,
    #[error("issuer is not allowed to issue this role")]
    IssuerNotAllowed,
    #[error("certificate chain too deep")]
    ChainTooDeep,
    #[error("a different certificate already uses this serial")]
    SerialConflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid public key")]
    BadPublicKey,
    #[error("signature does not verify")]
    BadSignature,
    #[error("decryption failed")]
    DecryptFailed,
    #[error("message type does not match envelope kind")]
    WrongEnvelopeKind,
    #[error("key share is addressed to another device")]
    NotForMe,
    #[error("key derivation failed or parameters out of range")]
    Kdf,
    #[error("passphrase must be at least 12 characters")]
    WeakPassphrase,
    #[error("wrong passphrase or damaged recovery kit")]
    WrongPassphrase,
    #[error("token is expired or invalid")]
    BadToken,
    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),
    #[error("certificate: {0}")]
    Cert(#[from] CertError),
}
