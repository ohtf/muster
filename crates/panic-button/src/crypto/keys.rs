// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Device keys, group keys and signatures.
//!
//! No function here draws randomness. Callers pass random bytes in, so
//! the `core` module stays deterministic under simulation (ADR 0002).

use std::fmt;

use crate::proto::envelope::context;
use crate::proto::{DeviceId, PracticeTag};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as KxPublic, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::CryptoError;

/// A device's long-term keys: Ed25519 for signing (the public key is the
/// device id) and X25519 for key exchange. Secrets are zeroized on drop.
pub struct DeviceKeys {
    signing: SigningKey,
    kx: StaticSecret,
}

impl DeviceKeys {
    /// Builds keys from two 32-byte secrets (from the OS RNG in production).
    pub fn from_secrets(signing_secret: [u8; 32], kx_secret: [u8; 32]) -> Self {
        let mut s = signing_secret;
        let mut k = kx_secret;
        let keys = DeviceKeys {
            signing: SigningKey::from_bytes(&s),
            kx: StaticSecret::from(k),
        };
        s.zeroize();
        k.zeroize();
        keys
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId(self.signing.verifying_key().to_bytes())
    }

    pub fn kx_pub(&self) -> [u8; 32] {
        KxPublic::from(&self.kx).to_bytes()
    }

    /// Signs `context || 0x00 || data`.
    pub fn sign(&self, ctx: &str, data: &[u8]) -> [u8; 64] {
        self.signing
            .sign(&context::signing_input(ctx, data))
            .to_bytes()
    }

    /// The secret halves, for persistence by the store. Handle with care.
    pub fn to_secrets(&self) -> ([u8; 32], [u8; 32]) {
        (self.signing.to_bytes(), self.kx.to_bytes())
    }

    pub(crate) fn kx_secret(&self) -> &StaticSecret {
        &self.kx
    }
}

impl fmt::Debug for DeviceKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceKeys")
            .field("device_id", &self.device_id())
            .finish_non_exhaustive()
    }
}

impl Clone for DeviceKeys {
    fn clone(&self) -> Self {
        let (s, k) = self.to_secrets();
        DeviceKeys::from_secrets(s, k)
    }
}

/// A signing-only key (practice root, enroller), zeroized on drop.
pub struct SigningSecret(SigningKey);

impl SigningSecret {
    pub fn from_secret(secret: [u8; 32]) -> Self {
        let mut s = secret;
        let k = SigningSecret(SigningKey::from_bytes(&s));
        s.zeroize();
        k
    }

    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub fn sign(&self, ctx: &str, data: &[u8]) -> [u8; 64] {
        self.0.sign(&context::signing_input(ctx, data)).to_bytes()
    }

    pub fn to_secret(&self) -> [u8; 32] {
        self.0.to_bytes()
    }
}

impl fmt::Debug for SigningSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SigningSecret(pub={})", DeviceId(self.public()).short())
    }
}

/// Verifies an Ed25519 signature over `context || 0x00 || data`, using
/// strict verification (rejects non-canonical and small-order keys).
pub fn verify(
    public: &[u8; 32],
    ctx: &str,
    data: &[u8],
    sig: &[u8; 64],
) -> Result<(), CryptoError> {
    let vk = VerifyingKey::from_bytes(public).map_err(|_| CryptoError::BadPublicKey)?;
    let sig = Signature::from_bytes(sig);
    vk.verify_strict(&context::signing_input(ctx, data), &sig)
        .map_err(|_| CryptoError::BadSignature)
}

/// A practice group key for one epoch. Zeroized on drop.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct GroupKey([u8; 32]);

impl GroupKey {
    pub fn from_bytes(b: [u8; 32]) -> Self {
        GroupKey(b)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// `BLAKE3::keyed_hash(group_key, "panic-button/v1/tag")[0..8]`.
    pub fn tag(&self) -> PracticeTag {
        let h = blake3::keyed_hash(&self.0, b"panic-button/v1/tag");
        let mut t = [0u8; 8];
        t.copy_from_slice(&h.as_bytes()[..8]);
        PracticeTag(t)
    }

    /// Deterministic tie-break for concurrent rotations: the key with the
    /// higher `BLAKE3(key)` wins (`spec/04-security.md`, step 5).
    pub fn rotation_rank(&self) -> [u8; 32] {
        *blake3::hash(&self.0).as_bytes()
    }
}

impl fmt::Debug for GroupKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupKey(tag={})", data_hex(&self.tag().0))
    }
}

fn data_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The 6-digit interactive enrolment code:
/// `decimal(BLAKE3(sign_pub || kx_pub)[0..4] as big-endian u32 mod 1_000_000)`,
/// zero-padded.
pub fn join_code(sign_pub: &[u8; 32], kx_pub: &[u8; 32]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(sign_pub);
    h.update(kx_pub);
    let d = h.finalize();
    let b = d.as_bytes();
    let n = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) % 1_000_000;
    format!("{n:06}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_with_context() {
        let k = DeviceKeys::from_secrets([1; 32], [2; 32]);
        let sig = k.sign(context::SEALED, b"hello");
        let id = k.device_id();
        assert!(verify(&id.0, context::SEALED, b"hello", &sig).is_ok());
        // Different context, data or key fails.
        assert!(verify(&id.0, context::OPEN, b"hello", &sig).is_err());
        assert!(verify(&id.0, context::SEALED, b"hellp", &sig).is_err());
        let other = DeviceKeys::from_secrets([3; 32], [2; 32]).device_id();
        assert!(verify(&other.0, context::SEALED, b"hello", &sig).is_err());
    }

    #[test]
    fn rfc8032_test_vector_1() {
        // RFC 8032 section 7.1, TEST 1 (empty message), checked through the
        // underlying library so our wrapper uses the standard algorithm.
        let sk: [u8; 32] =
            hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
                .expect("hex")
                .try_into()
                .expect("32");
        let k = SigningKey::from_bytes(&sk);
        assert_eq!(
            hex::encode(k.verifying_key().to_bytes()),
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        );
        assert_eq!(
            hex::encode(k.sign(b"").to_bytes()),
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        );
    }

    #[test]
    fn rfc7748_x25519_vector() {
        // RFC 7748 section 6.1 (Alice and Bob).
        let a: [u8; 32] =
            hex::decode("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")
                .expect("hex")
                .try_into()
                .expect("32");
        let b: [u8; 32] =
            hex::decode("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb")
                .expect("hex")
                .try_into()
                .expect("32");
        let alice = DeviceKeys::from_secrets([0; 32], a);
        let bob = DeviceKeys::from_secrets([0; 32], b);
        assert_eq!(
            hex::encode(alice.kx_pub()),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        let shared = alice
            .kx_secret()
            .diffie_hellman(&KxPublic::from(bob.kx_pub()));
        assert_eq!(
            hex::encode(shared.as_bytes()),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        );
    }

    #[test]
    fn tag_differs_by_key() {
        assert_ne!(
            GroupKey::from_bytes([1; 32]).tag(),
            GroupKey::from_bytes([2; 32]).tag()
        );
    }

    #[test]
    fn join_code_is_six_digits() {
        let c = join_code(&[1; 32], &[2; 32]);
        assert_eq!(c.len(), 6);
        assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        assert_ne!(c, join_code(&[1; 32], &[3; 32]));
    }

    #[test]
    fn debug_does_not_leak_secrets() {
        let k = DeviceKeys::from_secrets([0xab; 32], [0xcd; 32]);
        let d = format!("{k:?} {:?}", GroupKey::from_bytes([0xef; 32]));
        assert!(!d.contains("abab") && !d.contains("cdcd") && !d.contains("efef"));
    }
}
