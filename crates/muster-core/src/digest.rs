// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

//! Anti-entropy digests (`spec/03-protocol.md`, Anti-entropy).

use muster_proto::{CertSerial, MsgId};

/// `BLAKE3("muster/v1/ad" || sorted (alarm_id || msg_id) pairs)[0..16]`
/// over the raise and every event of each alarm in the digest window.
pub fn alarm_digest(mut pairs: Vec<(MsgId, MsgId)>) -> [u8; 16] {
    pairs.sort();
    let mut h = blake3::Hasher::new();
    h.update(b"muster/v1/ad");
    for (a, m) in &pairs {
        h.update(&a.0);
        h.update(&m.0);
    }
    trunc(h.finalize())
}

/// `BLAKE3("muster/v1/sd" || sorted cert serials || sorted revocation
/// msg_ids || config version (u64 BE) || current epoch (u64 BE))[0..16]`.
pub fn state_digest<'a>(
    serials: impl Iterator<Item = &'a CertSerial>,
    revocations: impl Iterator<Item = &'a MsgId>,
    config_version: u64,
    epoch: u64,
) -> [u8; 16] {
    let mut s: Vec<_> = serials.collect();
    s.sort();
    let mut r: Vec<_> = revocations.collect();
    r.sort();
    let mut h = blake3::Hasher::new();
    h.update(b"muster/v1/sd");
    for x in s {
        h.update(&x.0);
    }
    for x in r {
        h.update(&x.0);
    }
    h.update(&config_version.to_be_bytes());
    h.update(&epoch.to_be_bytes());
    trunc(h.finalize())
}

fn trunc(h: blake3::Hash) -> [u8; 16] {
    let mut o = [0u8; 16];
    o.copy_from_slice(&h.as_bytes()[..16]);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alarm_digest_is_order_independent() {
        let a = (MsgId([1; 16]), MsgId([2; 16]));
        let b = (MsgId([1; 16]), MsgId([3; 16]));
        assert_eq!(alarm_digest(vec![a, b]), alarm_digest(vec![b, a]));
        assert_ne!(alarm_digest(vec![a]), alarm_digest(vec![a, b]));
    }
}
