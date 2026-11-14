# ADR 0005: Group-encrypted envelopes with per-device signatures

- Status: Accepted (2026-11-14)

## Context

Alarms are mostly broadcast to many devices over multicast and UDP, where per-connection channels (TLS, Noise) do not fit. Messages must be confidential from non-members, attributable to a specific device, and resistant to forgery and replay. Revocation of a lost laptop must work.

## Decision

- Each message is signed by the sending device (Ed25519) and sealed with the practice group key for the current epoch (XChaCha20-Poly1305, random 24-byte nonces). A keyed BLAKE3 tag lets other practices' traffic be dropped before decryption.
- Device membership is defined by certificates chaining to an offline practice root key, through admin and enroller certs.
- Revocation is a signed, flooded message. It is followed by group key rotation delivered by per-device sealed KeyShares (X25519 + HKDF).
- Non-cascading revocation of issuers is the default, so an admin leaving does not de-enrol the devices they set up.

## Alternatives considered

- **Per-pair Noise sessions:** strong properties, but they do not work with multicast or broadcast, and they need N^2 sessions.
- **MLS (RFC 9420):** excellent group key management, but heavy for v1 and needs a delivery service ordering assumption that the mesh does not provide. Revisit if groups grow or forward secrecy within an epoch becomes a requirement.
- **Signatures only (no encryption):** simpler, but leaks incident details to anyone on the LAN.

## Consequences

- No forward secrecy within an epoch. This is acceptable given the low sensitivity of the content (room and name), and documented.
- An enrolled insider can raise attributable false alarms. This is accepted and managed socially, with an audit trail.
