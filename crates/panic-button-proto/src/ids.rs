// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Fixed-size identifiers.

use std::fmt;

use crate::cbor::{Reader, Writer};
use crate::error::DecodeError;

macro_rules! id_type {
    ($(#[$m:meta])* $name:ident, $n:expr) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name(pub [u8; $n]);

        impl $name {
            pub const LEN: usize = $n;

            pub fn as_bytes(&self) -> &[u8; $n] {
                &self.0
            }

            pub fn encode(&self, w: &mut Writer) {
                w.bytes(&self.0);
            }

            pub fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
                Ok(Self(r.fixed::<$n>()?))
            }

            pub fn to_hex(&self) -> String {
                data_encoding::HEXLOWER.encode(&self.0)
            }

            pub fn from_hex(s: &str) -> Option<Self> {
                let v = data_encoding::HEXLOWER_PERMISSIVE.decode(s.as_bytes()).ok()?;
                Some(Self(<[u8; $n]>::try_from(v.as_slice()).ok()?))
            }
        }

        impl From<[u8; $n]> for $name {
            fn from(b: [u8; $n]) -> Self {
                Self(b)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.to_hex())
            }
        }
    };
}

id_type!(
    /// A device id: the device's Ed25519 public key.
    DeviceId,
    32
);
id_type!(
    /// A practice id (16 random bytes).
    PracticeId,
    16
);
id_type!(
    /// A message id (16 random bytes). For AlarmRaised it is also the alarm id.
    MsgId,
    16
);
id_type!(
    /// A certificate serial number.
    CertSerial,
    16
);
id_type!(
    /// An enrolment token id.
    TokenId,
    16
);
id_type!(
    /// The 8-byte practice tag on sealed envelopes.
    PracticeTag,
    8
);

impl DeviceId {
    /// Short display form: the first 8 characters of the lowercase base32
    /// encoding (`spec/03-protocol.md`, Conventions).
    pub fn short(&self) -> String {
        let s = data_encoding::BASE32_NOPAD
            .encode(&self.0)
            .to_ascii_lowercase();
        s[..8].to_owned()
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.short())
    }
}

impl PracticeId {
    /// The all-zero id used in a JoinRequest to mean "any practice".
    pub const ANY: PracticeId = PracticeId([0; 16]);
}

impl fmt::Display for MsgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex()[..12])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_form_is_8_lowercase_base32_chars() {
        let id = DeviceId([0u8; 32]);
        assert_eq!(id.short(), "aaaaaaaa");
        let id = DeviceId([0xff; 32]);
        assert_eq!(id.short(), "77777777");
    }

    #[test]
    fn hex_round_trip() {
        let id = PracticeId([0xab; 16]);
        assert_eq!(PracticeId::from_hex(&id.to_hex()), Some(id));
        assert_eq!(PracticeId::from_hex("abcd"), None);
    }
}
