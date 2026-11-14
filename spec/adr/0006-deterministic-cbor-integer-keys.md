# ADR 0006: Deterministic CBOR with integer map keys

- Status: Accepted (2026-11-14)

## Context

Messages must fit in 1200-byte UDP datagrams, be signed (so encoding must be canonical), and evolve compatibly.

## Decision

- Use CBOR with RFC 8949 core deterministic encoding and unsigned-integer map keys.
- Ignore unknown keys. Reject duplicate keys and indefinite lengths.
- Pick **one** crate (`minicbor` with derive, or `ciborium`) during M1 and record the choice in this ADR. Prefer whichever makes it easiest to enforce deterministic encoding and strict decoding.

## Alternatives considered

- **Protobuf:** not canonical by default (signing needs care), and adds a schema toolchain.
- **JSON:** too large, and not canonical.
- **MessagePack:** viable, but has less standardised deterministic rules.

## Consequences

- The spec and golden vectors define the wire format independently of Rust types.

## Crate choice (M1, 2026-11-14)

`minicbor` (2.x), used only through its low-level `Decoder` for reading primitives. Strictness (definite lengths, integer keys, duplicate-key rejection, depth and length bounds, unknown-key skipping) is enforced in `muster-proto::cbor::Reader`, so it does not depend on crate defaults. Encoding uses a small writer in the same module that emits shortest-form heads and definite lengths, with map keys always written in ascending order. This was chosen over `ciborium` because `ciborium` decodes through serde's data model, where duplicate-key rejection and unknown-key skipping with these rules are harder to guarantee.
