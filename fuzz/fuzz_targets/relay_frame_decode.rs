// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

#![no_main]

use libfuzzer_sys::fuzz_target;
use panic_button::proto::relay::{Frame, decode_batch};

fuzz_target!(|data: &[u8]| {
    if let Ok(f) = Frame::from_bytes(data) {
        assert_eq!(Frame::from_bytes(&f.to_bytes()).expect("re-decodes"), f);
    }
    let _ = decode_batch(data);
});
