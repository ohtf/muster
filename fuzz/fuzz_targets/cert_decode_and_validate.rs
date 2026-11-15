// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

#![no_main]

use libfuzzer_sys::fuzz_target;
use panic_button_crypto::TrustStore;
use panic_button_proto::{Cbor, Cert, PracticeId};

fuzz_target!(|data: &[u8]| {
    // Interpret the input as a sequence of length-prefixed certs.
    let mut store = TrustStore::new(PracticeId([1; 16]), [2; 32]);
    let mut rest = data;
    while let Some((&n, tail)) = rest.split_first() {
        let n = usize::from(n).min(tail.len());
        let (item, tail) = tail.split_at(n);
        if let Ok(c) = Cert::from_cbor(item) {
            if let Ok((serial, _)) = store.add_cert(c) {
                let _ = store.validate(&serial, 1_763_000_000_000);
            }
        }
        rest = tail;
    }
});
