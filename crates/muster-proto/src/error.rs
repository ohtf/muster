// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

use thiserror::Error;

/// Why an item could not be decoded. Every variant means "drop it"; the
/// variant is kept only for diagnostics counters.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    #[error("malformed CBOR")]
    Malformed,
    #[error("indefinite-length item")]
    Indefinite,
    #[error("duplicate map key {0}")]
    DuplicateKey(u64),
    #[error("map key is not an unsigned integer")]
    NonIntegerKey,
    #[error("unexpected CBOR type")]
    UnexpectedType,
    #[error("missing required field {0}")]
    Missing(u64),
    #[error("wrong length for a fixed-size byte string")]
    WrongLength,
    #[error("item exceeds a size limit")]
    TooLarge,
    #[error("nesting too deep")]
    TooDeep,
    #[error("trailing bytes after item")]
    Trailing,
    #[error("unsupported envelope version {0}")]
    UnsupportedVersion(u64),
    #[error("unknown message type {0:#04x}")]
    UnknownType(u64),
    #[error("invalid value")]
    InvalidValue,
    #[error("bad text encoding")]
    BadEncoding,
}
