# ADR 0002: Rust, with a sans-IO core

- Status: Accepted (2026-11-14)

## Context

The same protocol logic must run on Windows, macOS, Linux, a headless server and, later, iOS and Android. Correctness must be demonstrated under hostile network conditions that are hard to reproduce on real networks.

## Decision

- Implement in Rust. Core protocol and alarm logic live in `muster-core`, which performs no I/O, reads no clocks and uses only injected randomness.
- Runtimes (desktop, headless, simulator, and later mobile through UniFFI) feed inputs and perform outputs.

## Alternatives considered

- **Go:** simpler networking and cross-compilation, but weaker for embedding in mobile apps and in a Tauri desktop shell.
- **.NET:** good on Windows and what commercial competitors use, but weaker for Linux desktop and embedded mobile, and a heavier runtime.
- **A core that performs I/O directly:** simpler at first, but makes deterministic simulation and property testing impractical.

## Consequences

- The simulator can run thousands of randomised network worlds per minute, deterministically, with shrinking.
- Slightly more boilerplate (Input and Output enums). This is accepted.
