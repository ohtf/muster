# ADR 0015: Rename from Muster to Panic Button

- Status: Accepted (2026-11-15), decided by Marcus Baw.

## Context

"Muster" was a working name. Before the first release, a check of the package channels found it already in use by unrelated projects: `muster`, `muster-cli` and `muster-store` on crates.io, and `muster-bin` and `muster-agent-bin` on the AUR. Nothing had been released or installed outside development, so renaming cost only a code change.

## Decision

1. The product is Panic Button. Packages, crates, commands and paths use `panic-button`: the `panic-button` CLI (crate `panic-button`), `panic-button-agent`, `panic-button-relay`, `panic-button-desktop`, and library crates `panic-button-proto`, `-crypto`, `-core`, `-store`, `-net`. All were free on crates.io, the AUR, Arch, Homebrew and Debian on the day.
2. The rename covers the wire protocol too: domain-separation labels (`panic-button/v1/...`), the mDNS service `_panic-button._udp`, the enrolment token prefix `panic-button-enrol-1:` and relay HTTP headers. With no deployed devices there is nothing to stay compatible with. The CBOR encoding of every message is unchanged (the golden vectors are byte-identical).
3. Environment variables use `PANIC_BUTTON_`, the app identifier is `org.ohtf.panic-button`, Windows data lives in `%ProgramData%\Panic Button`, and the licence addition is `LicenseRef-PanicButton-app-store-exception` (same text).
4. The repository moves to `ohtf/panic-button`; GitHub redirects the old URL.

## Consequences

- Earlier commits, ADRs and conversation logs say Muster; they are history and are not rewritten.
- "Panic Button" is a descriptive name. It is easy to understand, but harder to protect as a trade mark (R51).
