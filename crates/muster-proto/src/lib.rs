// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Muster wire protocol: message types and strict deterministic CBOR encoding (no I/O).

#![forbid(unsafe_code)]

pub mod cbor;
pub mod cert;
pub mod envelope;
pub mod error;
pub mod ids;
pub mod limits;
pub mod message;
pub mod relay;
pub mod settings;
pub mod token;

pub use cbor::Cbor;
pub use cert::{Cert, CertBody, Role};
pub use envelope::{Envelope, OpenEnvelope, SealedEnvelope, SignedMessage};
pub use error::DecodeError;
pub use ids::{CertSerial, DeviceId, MsgId, PracticeId, PracticeTag, TokenId};
pub use message::*;
pub use settings::{CueMode, DrillMode, PracticeSettings, SoundPattern};
pub use token::{NetworkOverrides, Token, TokenParseError};
