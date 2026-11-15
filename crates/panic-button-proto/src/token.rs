// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Enrolment token format (`spec/04-security.md`, Token enrolment).
//!
//! `panic-button-enrol-1:` followed by unpadded base64url of
//! `CBOR {0: version, 1: practice_id, 2: practice_name, 3: root_pub,
//! 4: enroller_cert, 5: enroller_secret, 6: epoch, 7: group_key,
//! 8: relay_urls, 9: seed_peers, 10: network overrides, 11: token_id,
//! 12: expires_at}`.
//!
//! Key 13 (extension) carries the enroller's issuer chain.
//!
//! Network overrides (key 10) are `{0: port, 1: multicast_group bstr(4),
//! 2: multicast_ttl}`, every entry optional.

use std::fmt;

use zeroize::Zeroize;

use crate::cbor::{Cbor, MapBuilder, Reader, Writer, req};
use crate::error::DecodeError;
use crate::ids::{PracticeId, TokenId};
use crate::limits;

pub const TOKEN_PREFIX: &str = "panic-button-enrol-1:";
pub const TOKEN_VERSION: u64 = 1;
/// Upper bound on the encoded token string, to bound decoding work.
pub const TOKEN_STRING_MAX: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkOverrides {
    pub port: Option<u16>,
    pub multicast_group: Option<[u8; 4]>,
    pub multicast_ttl: Option<u8>,
}

impl Cbor for NetworkOverrides {
    fn encode(&self, w: &mut Writer) {
        let mut m = MapBuilder::new();
        if let Some(p) = self.port {
            m.entry(0, |w| {
                w.u64(u64::from(p));
            });
        }
        if let Some(g) = &self.multicast_group {
            m.entry(1, |w| {
                w.bytes(g);
            });
        }
        if let Some(t) = self.multicast_ttl {
            m.entry(2, |w| {
                w.u64(u64::from(t));
            });
        }
        m.write(w);
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut n = NetworkOverrides::default();
        r.map(|k, r| {
            match k {
                0 => n.port = Some(r.u16()?),
                1 => n.multicast_group = Some(r.fixed::<4>()?),
                2 => n.multicast_ttl = Some(r.u8()?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(n)
    }
}

/// A decoded enrolment token. Holds secrets: zeroized on drop, redacted in
/// `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    pub version: u64,
    pub practice_id: PracticeId,
    pub practice_name: String,
    pub root_pub: [u8; 32],
    /// Raw signed enroller cert.
    pub enroller_cert: Vec<u8>,
    pub enroller_secret: [u8; 32],
    pub epoch: u64,
    pub group_key: [u8; 32],
    pub relay_urls: Vec<String>,
    pub seed_peers: Vec<String>,
    pub network: NetworkOverrides,
    pub token_id: TokenId,
    pub expires_at: u64,
    /// Key 13 (extension): raw certs from the enroller cert's issuer up to
    /// but not including the root, so a newly enrolled device can validate
    /// its own chain before it hears from an admin.
    pub issuer_chain: Vec<Vec<u8>>,
}

impl Drop for Token {
    fn drop(&mut self) {
        self.enroller_secret.zeroize();
        self.group_key.zeroize();
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Token")
            .field("practice_id", &self.practice_id)
            .field("practice_name", &self.practice_name)
            .field("token_id", &self.token_id)
            .field("epoch", &self.epoch)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenParseError {
    #[error("not a Panic Button enrolment token (missing prefix)")]
    Prefix,
    #[error("token is too long")]
    TooLong,
    #[error("token is not valid base64url")]
    Base64,
    #[error("token content is invalid: {0}")]
    Decode(#[from] DecodeError),
    #[error("unsupported token version")]
    Version,
}

impl Token {
    /// Encodes as the `panic-button-enrol-1:...` string form.
    pub fn to_token_string(&self) -> String {
        let mut b = self.to_cbor();
        let s = format!(
            "{TOKEN_PREFIX}{}",
            data_encoding::BASE64URL_NOPAD.encode(&b)
        );
        b.zeroize();
        s
    }

    /// Parses the string form. Surrounding whitespace is ignored so tokens
    /// pasted from files or emails work.
    pub fn parse(s: &str) -> Result<Self, TokenParseError> {
        let s = s.trim();
        if s.len() > TOKEN_STRING_MAX {
            return Err(TokenParseError::TooLong);
        }
        let rest = s
            .strip_prefix(TOKEN_PREFIX)
            .ok_or(TokenParseError::Prefix)?;
        let mut b = data_encoding::BASE64URL_NOPAD
            .decode(rest.as_bytes())
            .map_err(|_| TokenParseError::Base64)?;
        let t = Token::from_cbor(&b);
        b.zeroize();
        let t = t?;
        if t.version != TOKEN_VERSION {
            return Err(TokenParseError::Version);
        }
        Ok(t)
    }
}

fn text_list(w: &mut Writer, v: &[String]) {
    w.array(v.len());
    for s in v {
        w.text_full(s);
    }
}

impl Cbor for Token {
    fn encode(&self, w: &mut Writer) {
        w.map(if self.issuer_chain.is_empty() { 13 } else { 14 });
        w.u64(0).u64(self.version);
        w.u64(1).bytes(&self.practice_id.0);
        w.u64(2).text(&self.practice_name);
        w.u64(3).bytes(&self.root_pub);
        w.u64(4).bytes(&self.enroller_cert);
        w.u64(5).bytes(&self.enroller_secret);
        w.u64(6).u64(self.epoch);
        w.u64(7).bytes(&self.group_key);
        w.u64(8);
        text_list(w, &self.relay_urls);
        w.u64(9);
        text_list(w, &self.seed_peers);
        w.u64(10);
        self.network.encode(w);
        w.u64(11).bytes(&self.token_id.0);
        w.u64(12).u64(self.expires_at);
        if !self.issuer_chain.is_empty() {
            w.u64(13).array(self.issuer_chain.len());
            for c in &self.issuer_chain {
                w.bytes(c);
            }
        }
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut version = None;
        let mut practice_id = None;
        let mut name = None;
        let mut root = None;
        let mut cert = None;
        let mut secret = None;
        let mut epoch = None;
        let mut gk = None;
        let mut relays = None;
        let mut seeds = None;
        let mut net = None;
        let mut tid = None;
        let mut exp = None;
        let mut chain = None;
        let list = |r: &mut Reader<'_>| r.array_of(64, |r| r.text_max(limits::CONFIG_STRING_MAX));
        let mapped = r.map(|k, r| {
            match k {
                0 => version = Some(r.u64()?),
                1 => practice_id = Some(PracticeId::decode(r)?),
                2 => name = Some(r.text()?),
                3 => root = Some(r.fixed::<32>()?),
                4 => cert = Some(r.bytes(4096)?.to_vec()),
                5 => secret = Some(r.fixed::<32>()?),
                6 => epoch = Some(r.u64()?),
                7 => gk = Some(r.fixed::<32>()?),
                8 => relays = Some(list(r)?),
                9 => seeds = Some(list(r)?),
                10 => net = Some(NetworkOverrides::decode(r)?),
                11 => tid = Some(TokenId::decode(r)?),
                12 => exp = Some(r.u64()?),
                13 => {
                    chain =
                        Some(r.array_of(limits::CERT_CHAIN_MAX, |r| Ok(r.bytes(4096)?.to_vec()))?)
                }
                _ => return Ok(false),
            }
            Ok(true)
        });
        let result = || -> Result<Token, DecodeError> {
            Ok(Token {
                version: req(version, 0)?,
                practice_id: req(practice_id, 1)?,
                practice_name: req(name, 2)?,
                root_pub: req(root, 3)?,
                enroller_cert: req(cert.take(), 4)?,
                enroller_secret: req(secret, 5)?,
                epoch: req(epoch, 6)?,
                group_key: req(gk, 7)?,
                relay_urls: relays.take().unwrap_or_default(),
                seed_peers: seeds.take().unwrap_or_default(),
                network: net.take().unwrap_or_default(),
                token_id: req(tid, 11)?,
                expires_at: req(exp, 12)?,
                issuer_chain: chain.take().unwrap_or_default(),
            })
        };
        let out = mapped.and_then(|()| result());
        if let Some(mut s) = secret {
            s.zeroize();
        }
        if let Some(mut g) = gk {
            g.zeroize();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Token {
        Token {
            version: 1,
            practice_id: PracticeId([1; 16]),
            practice_name: "Riverside Surgery".into(),
            root_pub: [2; 32],
            enroller_cert: vec![0xa0],
            enroller_secret: [3; 32],
            epoch: 4,
            group_key: [5; 32],
            relay_urls: vec!["wss://relay.example/v1/ws".into()],
            seed_peers: vec!["10.0.0.1:47474".into()],
            network: NetworkOverrides {
                port: Some(47475),
                multicast_group: None,
                multicast_ttl: Some(2),
            },
            token_id: TokenId([6; 16]),
            expires_at: 1_800_000_000_000,
            issuer_chain: vec![vec![0xa0]],
        }
    }

    #[test]
    fn string_round_trip() {
        let t = sample();
        let s = t.to_token_string();
        assert!(s.starts_with(TOKEN_PREFIX));
        assert_eq!(Token::parse(&format!("  {s}\n")), Ok(t));
    }

    #[test]
    fn rejects_bad_tokens() {
        assert_eq!(Token::parse("hello"), Err(TokenParseError::Prefix));
        assert_eq!(
            Token::parse("panic-button-enrol-1:!!!"),
            Err(TokenParseError::Base64)
        );
        assert!(matches!(
            Token::parse("panic-button-enrol-1:oA"),
            Err(TokenParseError::Decode(DecodeError::Missing(_)))
        ));
    }

    #[test]
    fn debug_redacts_secrets() {
        let d = format!("{:?}", sample());
        assert!(!d.contains("[3, 3"), "{d}");
        assert!(!d.contains("[5, 5"), "{d}");
    }
}
