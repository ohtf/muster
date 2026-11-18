# Golden vectors

Hex-encoded CBOR for one sample of every Panic Button message body (and a certificate), produced by `crates/panic-button/tests/messages.rs`. Other implementations can decode these and compare the result with the sample values in that file.

Regenerate with `PANIC_BUTTON_BLESS=1 cargo test -p panic-button --test messages`. A change to any vector is a wire-format change: review it, and explain it in the commit.
