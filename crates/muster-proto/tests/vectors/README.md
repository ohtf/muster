# Golden vectors

Hex-encoded CBOR for one sample of every Muster message body (and a certificate), produced by `crates/muster-proto/tests/messages.rs`. Other implementations can decode these and compare the result with the sample values in that file.

Regenerate with `MUSTER_BLESS=1 cargo test -p muster-proto --test messages`. A change to any vector is a wire-format change: review it, and explain it in the commit.
