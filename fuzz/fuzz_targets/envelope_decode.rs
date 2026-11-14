// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

#![no_main]

use libfuzzer_sys::fuzz_target;
use muster_proto::{Cbor, Envelope};

fuzz_target!(|data: &[u8]| {
    if let Ok(e) = Envelope::from_cbor(data) {
        // Anything that decodes must re-encode and decode to the same value.
        let again = Envelope::from_cbor(&e.to_cbor()).expect("re-decodes");
        assert_eq!(again, e);
    }
});
