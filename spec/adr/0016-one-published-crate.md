# ADR 0016: One published crate

- Status: Accepted (2026-11-18), decided by Marcus Baw.

## Context

The Rust code was split into nine workspace crates: `panic-button-proto`, `-crypto`, `-core`, `-store`, `-net`, `-agent`, `-relay`, the `panic-button` CLI and the unpublished simulator. crates.io only accepts a crate whose dependencies are also on crates.io, so publishing the CLI meant publishing seven internal crates too. For v0.1.0 that cost a new-crate rate limit, eight crates to release in order, and 16 trusted-publisher entries, for libraries nobody outside the project uses.

The split did enforce three boundaries at compile time: `proto`, `crypto` and `core` do no I/O (so the simulator is deterministic); each layer uses only the layers below; and the relay uses only `proto` and `crypto`.

## Decision

1. One published crate, `panic-button`. The former crates are modules in it (`proto`, `crypto`, `core`, `store`, `net`, `agent`, `relay`, `cli`), and it has three programs: `panic-button`, `panic-button-agent` and `panic-button-relay`. `cargo install panic-button` installs all three.
2. The relay and its web server dependency sit behind the `relay` feature, on by default. The desktop app and the simulator use the library without it.
3. `crates/panic-button/tests/boundaries.rs` replaces the compile-time boundaries: it fails if a module uses a module it should not, or if `proto`, `crypto` or `core` mention I/O, the clock or system randomness.
4. `#![forbid(unsafe_code)]` applies to the whole crate. Platform FFI stays in the desktop app.
5. The simulator and the desktop app stay separate, unpublished workspace crates.
6. A part becomes its own crate again only when it has real reuse value outside the project, for example `proto` for a second implementation (house style `library-extraction.md`).
7. The seven former crates' 0.1.0 releases are yanked on crates.io. The next release of `panic-button` is 0.2.0.

## Consequences

- One crates.io entry, one trusted-publisher configuration per workflow, and one cargo-dist package: one archive, install script and Homebrew formula (`panic-button-cli`) per platform with all three programs. The release workflow removes the old `panic-button-agent` and `panic-button-relay` formulae from the tap.
- The boundary test reads source text, so it is stricter than the old crate graph in one way (a forbidden name in a comment fails) and weaker in another (a path it does not know about could slip through). Code review covers the rest.
- Until 0.2.0 is released, `cargo install panic-button` fails, because 0.1.0 depends on the yanked crates.
