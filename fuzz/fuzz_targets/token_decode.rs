// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception

#![no_main]

use libfuzzer_sys::fuzz_target;
use muster_proto::Token;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Token::parse(s);
        let with_prefix = format!("{}{s}", muster_proto::token::TOKEN_PREFIX);
        if let Ok(t) = Token::parse(&with_prefix) {
            let once = t.to_token_string();
            let again = Token::parse(&once).expect("re-parses");
            assert_eq!(again.to_token_string(), once);
        }
    }
});
