// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Strict CBOR reading and deterministic CBOR writing (ADR 0006).
//!
//! Writing produces RFC 8949 section 4.2.1 core deterministic encoding for
//! the subset Muster uses: shortest-form unsigned integers and lengths,
//! definite lengths only, and map keys written in ascending order (callers
//! write keys in ascending order; [`Writer::map`] checks this in debug builds).
//!
//! Reading uses `minicbor`'s decoder for primitives and enforces the rules in
//! `spec/03-protocol.md`: map keys are unsigned integers, duplicate keys and
//! indefinite-length items are rejected, unknown keys are skipped, nesting
//! depth and item counts are bounded, and nothing panics on untrusted input.

use minicbor::data::Type;

use crate::error::DecodeError;
use crate::limits;

/// Maximum nesting depth of any decoded item. Muster messages nest at most
/// about four levels; the bound exists to stop stack exhaustion.
pub const MAX_DEPTH: usize = 16;

const MAJOR_UINT: u8 = 0;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_ARRAY: u8 = 4;
const MAJOR_MAP: u8 = 5;
const SIMPLE_FALSE: u8 = 0xf4;
const SIMPLE_TRUE: u8 = 0xf5;

/// A deterministic CBOR writer.
#[derive(Debug, Default, Clone)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    fn header(&mut self, major: u8, n: u64) {
        let m = major << 5;
        if n < 24 {
            self.buf.push(m | n as u8);
        } else if n <= u64::from(u8::MAX) {
            self.buf.push(m | 24);
            self.buf.push(n as u8);
        } else if n <= u64::from(u16::MAX) {
            self.buf.push(m | 25);
            self.buf.extend_from_slice(&(n as u16).to_be_bytes());
        } else if n <= u64::from(u32::MAX) {
            self.buf.push(m | 26);
            self.buf.extend_from_slice(&(n as u32).to_be_bytes());
        } else {
            self.buf.push(m | 27);
            self.buf.extend_from_slice(&n.to_be_bytes());
        }
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.header(MAJOR_UINT, v);
        self
    }

    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.buf.push(if v { SIMPLE_TRUE } else { SIMPLE_FALSE });
        self
    }

    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.header(MAJOR_BYTES, v.len() as u64);
        self.buf.extend_from_slice(v);
        self
    }

    /// Writes a text string, truncated to [`limits::STRING_SEND_MAX`] bytes on
    /// a UTF-8 character boundary.
    pub fn text(&mut self, v: &str) -> &mut Self {
        let v = truncate_utf8(v, limits::STRING_SEND_MAX);
        self.header(MAJOR_TEXT, v.len() as u64);
        self.buf.extend_from_slice(v.as_bytes());
        self
    }

    /// Writes a text string without truncation (for URLs and other
    /// configuration values that must not be silently shortened).
    pub fn text_full(&mut self, v: &str) -> &mut Self {
        self.header(MAJOR_TEXT, v.len() as u64);
        self.buf.extend_from_slice(v.as_bytes());
        self
    }

    pub fn array(&mut self, len: usize) -> &mut Self {
        self.header(MAJOR_ARRAY, len as u64);
        self
    }

    /// Writes a map header for `len` entries. The caller writes `len`
    /// key/value pairs with keys in strictly ascending order.
    pub fn map(&mut self, len: usize) -> &mut Self {
        self.header(MAJOR_MAP, len as u64);
        self
    }

    /// Appends already-encoded CBOR (an embedded item).
    pub fn raw(&mut self, encoded: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(encoded);
        self
    }
}

/// Truncate `s` to at most `max` bytes without splitting a character.
pub fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Helper for writing a map whose optional entries are only known at
/// runtime: collects `(key, encoded value)` pairs, then writes them sorted.
#[derive(Debug, Default)]
pub struct MapBuilder {
    entries: Vec<(u64, Vec<u8>)>,
}

impl MapBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entry(&mut self, key: u64, f: impl FnOnce(&mut Writer)) -> &mut Self {
        let mut w = Writer::new();
        f(&mut w);
        self.entries.push((key, w.into_bytes()));
        self
    }

    pub fn write(mut self, w: &mut Writer) {
        self.entries.sort_by_key(|(k, _)| *k);
        debug_assert!(
            self.entries.windows(2).all(|p| p[0].0 < p[1].0),
            "duplicate key written"
        );
        w.map(self.entries.len());
        for (k, v) in &self.entries {
            w.u64(*k);
            w.raw(v);
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut w = Writer::new();
        self.write(&mut w);
        w.into_bytes()
    }
}

/// A strict CBOR reader over untrusted bytes.
#[derive(Debug)]
pub struct Reader<'b> {
    d: minicbor::Decoder<'b>,
    depth: usize,
}

impl<'b> Reader<'b> {
    pub fn new(bytes: &'b [u8]) -> Self {
        Self {
            d: minicbor::Decoder::new(bytes),
            depth: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.d.input().len().saturating_sub(self.d.position())
    }

    pub fn position(&self) -> usize {
        self.d.position()
    }

    pub fn input(&self) -> &'b [u8] {
        self.d.input()
    }

    /// Fails unless every input byte has been consumed.
    pub fn finish(&self) -> Result<(), DecodeError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(DecodeError::Trailing)
        }
    }

    fn datatype(&self) -> Result<Type, DecodeError> {
        self.d.datatype().map_err(|_| DecodeError::Malformed)
    }

    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        match self.datatype()? {
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                self.d.u64().map_err(|_| DecodeError::Malformed)
            }
            _ => Err(DecodeError::UnexpectedType),
        }
    }

    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        u16::try_from(self.u64()?).map_err(|_| DecodeError::InvalidValue)
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        u8::try_from(self.u64()?).map_err(|_| DecodeError::InvalidValue)
    }

    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.datatype()? {
            Type::Bool => self.d.bool().map_err(|_| DecodeError::Malformed),
            _ => Err(DecodeError::UnexpectedType),
        }
    }

    /// A definite-length byte string of at most `max` bytes.
    pub fn bytes(&mut self, max: usize) -> Result<&'b [u8], DecodeError> {
        match self.datatype()? {
            Type::Bytes => {}
            Type::BytesIndef => return Err(DecodeError::Indefinite),
            _ => return Err(DecodeError::UnexpectedType),
        }
        let b = self.d.bytes().map_err(|_| DecodeError::Malformed)?;
        if b.len() > max {
            return Err(DecodeError::TooLarge);
        }
        Ok(b)
    }

    /// A byte string of exactly `N` bytes.
    pub fn fixed<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let b = self.bytes(N)?;
        <[u8; N]>::try_from(b).map_err(|_| DecodeError::WrongLength)
    }

    /// A display string (label, room, name): rejected over
    /// [`limits::STRING_RECV_MAX`] bytes.
    pub fn text(&mut self) -> Result<String, DecodeError> {
        self.text_max(limits::STRING_RECV_MAX)
    }

    pub fn text_max(&mut self, max: usize) -> Result<String, DecodeError> {
        match self.datatype()? {
            Type::String => {}
            Type::StringIndef => return Err(DecodeError::Indefinite),
            _ => return Err(DecodeError::UnexpectedType),
        }
        let s = self.d.str().map_err(|_| DecodeError::Malformed)?;
        if s.len() > max {
            return Err(DecodeError::TooLarge);
        }
        Ok(s.to_owned())
    }

    /// Reads a definite array header. `max` bounds the item count.
    pub fn array_len(&mut self, max: usize) -> Result<usize, DecodeError> {
        match self.datatype()? {
            Type::Array => {}
            Type::ArrayIndef => return Err(DecodeError::Indefinite),
            _ => return Err(DecodeError::UnexpectedType),
        }
        let n = self
            .d
            .array()
            .map_err(|_| DecodeError::Malformed)?
            .ok_or(DecodeError::Indefinite)?;
        let n = usize::try_from(n).map_err(|_| DecodeError::TooLarge)?;
        // Every item takes at least one byte.
        if n > max || n > self.remaining() {
            return Err(DecodeError::TooLarge);
        }
        Ok(n)
    }

    /// Decodes an array whose items are each decoded by `f`.
    pub fn array_of<T>(
        &mut self,
        max: usize,
        mut f: impl FnMut(&mut Self) -> Result<T, DecodeError>,
    ) -> Result<Vec<T>, DecodeError> {
        let n = self.array_len(max)?;
        self.enter()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(f(self)?);
        }
        self.leave();
        Ok(out)
    }

    fn enter(&mut self) -> Result<(), DecodeError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(DecodeError::TooDeep)
        } else {
            Ok(())
        }
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Decodes a map with unsigned integer keys. For each entry `f` is called
    /// with the key; it either decodes the value and returns `Ok(true)`, or
    /// returns `Ok(false)` without consuming anything, in which case the
    /// value is skipped (unknown keys are ignored). Duplicate keys are
    /// rejected.
    pub fn map(
        &mut self,
        mut f: impl FnMut(u64, &mut Self) -> Result<bool, DecodeError>,
    ) -> Result<(), DecodeError> {
        match self.datatype()? {
            Type::Map => {}
            Type::MapIndef => return Err(DecodeError::Indefinite),
            _ => return Err(DecodeError::UnexpectedType),
        }
        let n = self
            .d
            .map()
            .map_err(|_| DecodeError::Malformed)?
            .ok_or(DecodeError::Indefinite)?;
        let n = usize::try_from(n).map_err(|_| DecodeError::TooLarge)?;
        if n.saturating_mul(2) > self.remaining() {
            return Err(DecodeError::TooLarge);
        }
        self.enter()?;
        let mut seen: Vec<u64> = Vec::with_capacity(n.min(32));
        for _ in 0..n {
            let key = self.u64().map_err(|e| match e {
                DecodeError::UnexpectedType => DecodeError::NonIntegerKey,
                e => e,
            })?;
            if seen.contains(&key) {
                return Err(DecodeError::DuplicateKey(key));
            }
            seen.push(key);
            let before = self.position();
            if !f(key, self)? {
                if self.position() != before {
                    return Err(DecodeError::Malformed);
                }
                self.skip()?;
            }
        }
        self.leave();
        Ok(())
    }

    /// Skips one item, applying the same strictness rules as decoding:
    /// definite lengths only, integer map keys, no duplicate keys, bounded
    /// depth. Tags are rejected (Muster does not use them).
    pub fn skip(&mut self) -> Result<(), DecodeError> {
        match self.datatype()? {
            Type::BytesIndef | Type::StringIndef | Type::ArrayIndef | Type::MapIndef => {
                Err(DecodeError::Indefinite)
            }
            Type::Array => {
                let n = self.array_len(usize::MAX)?;
                self.enter()?;
                for _ in 0..n {
                    self.skip()?;
                }
                self.leave();
                Ok(())
            }
            Type::Map => self.map(|_, _| Ok(false)),
            Type::Tag | Type::Break | Type::Int | Type::Unknown(_) => {
                Err(DecodeError::UnexpectedType)
            }
            _ => self.d.skip().map_err(|_| DecodeError::Malformed),
        }
    }

    /// Skips one item and returns its raw encoded bytes.
    pub fn raw_item(&mut self) -> Result<&'b [u8], DecodeError> {
        let start = self.position();
        self.skip()?;
        let end = self.position();
        self.input().get(start..end).ok_or(DecodeError::Malformed)
    }
}

/// Unwraps a required field.
pub fn req<T>(v: Option<T>, key: u64) -> Result<T, DecodeError> {
    v.ok_or(DecodeError::Missing(key))
}

/// Encoding and decoding of a self-contained CBOR item.
pub trait Cbor: Sized {
    fn encode(&self, w: &mut Writer);
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError>;

    fn to_cbor(&self) -> Vec<u8> {
        let mut w = Writer::new();
        self.encode(&mut w);
        w.into_bytes()
    }

    /// Decodes exactly one item occupying all of `bytes`.
    fn from_cbor(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let v = Self::decode(&mut r)?;
        r.finish()?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_form_integers() {
        // RFC 8949 appendix A examples.
        let cases: &[(u64, &str)] = &[
            (0, "00"),
            (23, "17"),
            (24, "1818"),
            (255, "18ff"),
            (256, "190100"),
            (65535, "19ffff"),
            (65536, "1a00010000"),
            (1_000_000, "1a000f4240"),
            (1_000_000_000_000, "1b000000e8d4a51000"),
            (u64::MAX, "1bffffffffffffffff"),
        ];
        for (v, hex) in cases {
            let mut w = Writer::new();
            w.u64(*v);
            assert_eq!(hex::encode(w.into_bytes()), *hex, "{v}");
        }
    }

    #[test]
    fn map_builder_sorts_keys() {
        let mut m = MapBuilder::new();
        m.entry(10, |w| {
            w.u64(1);
        });
        m.entry(2, |w| {
            w.bool(true);
        });
        assert_eq!(hex::encode(m.into_bytes()), "a202f50a01");
    }

    #[test]
    fn text_is_truncated_on_char_boundary() {
        let s = "é".repeat(40); // 80 bytes
        let mut w = Writer::new();
        w.text(&s);
        let b = w.into_bytes();
        let mut r = Reader::new(&b);
        let t = r.text().expect("decodes");
        assert_eq!(t.len(), 64);
    }

    #[test]
    fn rejects_duplicate_keys() {
        let b = hex::decode("a2000100 02".replace(' ', "")).expect("hex");
        let mut r = Reader::new(&b);
        assert_eq!(r.map(|_, _| Ok(false)), Err(DecodeError::DuplicateKey(0)));
    }

    #[test]
    fn rejects_indefinite_items() {
        for hex in ["bf00ff", "9f01ff", "5f4101ff", "7f6161ff"] {
            let b = hex::decode(hex).expect("hex");
            let mut r = Reader::new(&b);
            assert_eq!(r.skip(), Err(DecodeError::Indefinite), "{hex}");
        }
    }

    #[test]
    fn rejects_non_integer_keys() {
        let b = hex::decode("a1616101").expect("hex");
        let mut r = Reader::new(&b);
        assert_eq!(r.map(|_, _| Ok(false)), Err(DecodeError::NonIntegerKey));
    }

    #[test]
    fn rejects_deep_nesting() {
        let mut b = vec![0x81; 100];
        b.push(0x00);
        let mut r = Reader::new(&b);
        assert_eq!(r.skip(), Err(DecodeError::TooDeep));
    }

    #[test]
    fn rejects_huge_claimed_lengths() {
        // Array claiming 2^32 items with no content.
        let b = hex::decode("9b0000000100000000").expect("hex");
        let mut r = Reader::new(&b);
        assert_eq!(r.skip(), Err(DecodeError::TooLarge));
    }

    #[test]
    fn skips_unknown_keys() {
        // {0: 1, 99: [1, {5: h'00'}], 1: 2}
        let b = hex::decode("a30001186382 01a1054100 0102".replace(' ', "")).expect("hex");
        let mut r = Reader::new(&b);
        let mut got = vec![];
        r.map(|k, r| {
            if k == 99 {
                return Ok(false);
            }
            got.push((k, r.u64()?));
            Ok(true)
        })
        .expect("decodes");
        r.finish().expect("no trailing");
        assert_eq!(got, vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn rejects_tags() {
        let b = hex::decode("c11a514b67b0").expect("hex");
        let mut r = Reader::new(&b);
        assert_eq!(r.skip(), Err(DecodeError::UnexpectedType));
    }
}
