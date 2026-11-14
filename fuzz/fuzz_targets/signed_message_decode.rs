// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

#![no_main]

use libfuzzer_sys::fuzz_target;
use muster_proto::{Body, Cbor, SignedMessage};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = SignedMessage::from_cbor(data) {
        let _ = muster_crypto::verify_signed(&s, false);
        let _ = muster_crypto::verify_signed(&s, true);
    }
    if let Ok(b) = Body::from_cbor(data) {
        // Display strings are truncated on send, so compare after one
        // re-encoding: encoding must then be stable.
        let once = b.to_cbor();
        let again = Body::from_cbor(&once).expect("re-decodes");
        assert_eq!(again.to_cbor(), once);
    }
});
