// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Enrolment token creation and use (`spec/04-security.md`, flow B).

use muster_proto::cert::CERT_VERSION;
use muster_proto::{
    Cbor, Cert, CertBody, CertSerial, NetworkOverrides, PracticeId, Role, Token, TokenId,
};

use crate::CryptoError;
use crate::cert::{ENROLLER_MAX_LIFETIME_MS, issue_cert};
use crate::keys::{DeviceKeys, GroupKey, SigningSecret};

/// Default token lifetime: 30 days.
pub const DEFAULT_TOKEN_LIFETIME_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// What an admin chooses when creating a token.
#[derive(Debug, Clone)]
pub struct TokenRequest {
    pub practice_id: PracticeId,
    pub practice_name: String,
    pub root_pub: [u8; 32],
    pub label: String,
    pub token_id: TokenId,
    pub enroller_serial: CertSerial,
    pub issued_at: u64,
    pub expires_at: u64,
    pub epoch: u64,
    pub relay_urls: Vec<String>,
    pub seed_peers: Vec<String>,
    pub network: NetworkOverrides,
}

/// Creates a token: issues an enroller cert (signed by the admin device) for
/// a fresh enroller key and bundles everything a device needs to join.
pub fn create_token(
    admin: &DeviceKeys,
    enroller_secret: [u8; 32],
    group_key: &GroupKey,
    req: TokenRequest,
) -> Result<(Token, Cert), CryptoError> {
    if req.expires_at <= req.issued_at || req.expires_at - req.issued_at > ENROLLER_MAX_LIFETIME_MS
    {
        return Err(CryptoError::BadToken);
    }
    let enroller = SigningSecret::from_secret(enroller_secret);
    let cert = issue_cert(
        admin,
        CertBody {
            version: CERT_VERSION,
            practice_id: req.practice_id,
            serial: req.enroller_serial,
            subject: muster_proto::DeviceId(enroller.public()),
            subject_kx: [0; 32],
            role: Role::Enroller,
            label: req.label,
            issued_at: req.issued_at,
            expires_at: req.expires_at,
            issuer: [0; 32],
            token_id: Some(req.token_id),
        },
    );
    let token = Token {
        version: muster_proto::token::TOKEN_VERSION,
        practice_id: req.practice_id,
        practice_name: req.practice_name,
        root_pub: req.root_pub,
        enroller_cert: cert.to_cbor(),
        enroller_secret: enroller.to_secret(),
        epoch: req.epoch,
        group_key: *group_key.as_bytes(),
        relay_urls: req.relay_urls,
        seed_peers: req.seed_peers,
        network: req.network,
        token_id: req.token_id,
        expires_at: req.expires_at,
    };
    Ok((token, cert))
}

/// Self-issues a member cert for `device` using the token's enroller key.
/// Refuses an expired token (judged without tolerance, so a device never
/// enrols with a cert that peers would reject).
pub fn enrol_with_token(
    token: &Token,
    device: &DeviceKeys,
    label: &str,
    serial: CertSerial,
    now: u64,
) -> Result<Cert, CryptoError> {
    if now > token.expires_at {
        return Err(CryptoError::BadToken);
    }
    let enroller = SigningSecret::from_secret(token.enroller_secret);
    let enroller_cert = Cert::from_cbor(&token.enroller_cert)?;
    let eb = enroller_cert.body()?;
    if eb.subject.0 != enroller.public() || eb.practice_id != token.practice_id {
        return Err(CryptoError::BadToken);
    }
    Ok(issue_cert(
        &enroller,
        CertBody {
            version: CERT_VERSION,
            practice_id: token.practice_id,
            serial,
            subject: device.device_id(),
            subject_kx: device.kx_pub(),
            role: Role::Member,
            label: label.to_owned(),
            issued_at: now,
            expires_at: 0,
            issuer: [0; 32],
            token_id: Some(token.token_id),
        },
    ))
}
