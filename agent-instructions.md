# Agent instructions

Muster is an open-source, LAN-first staff panic alarm for GP practices, stewarded by OHTF. It is not a monitored alarm, not a 999 route and never handles patient data. People's safety depends on this software working. When in doubt, favour (a) delivering the alarm and (b) honestly reporting when it might not be delivered.

This file is the entry point for AI coding agents (`AGENTS.md` and `CLAUDE.md` point here). Read it before changing anything.

## Read first

1. [README.md](README.md).
2. [spec/README.md](spec/README.md), then `spec/00` to `spec/09` in order, then [spec/adr/](spec/adr/).
3. [SAFETY.md](SAFETY.md) and [safety/hazard-log.md](safety/hazard-log.md).
4. [spec/queries.md](spec/queries.md) for open questions.
5. [pacharanero/house-style](https://github.com/pacharanero/house-style/blob/main/AGENTS.md) - cross-repo standards, adopted here. The roadmap follows its [checkbox and stable `R<n>` code conventions](https://github.com/pacharanero/house-style/blob/main/specs.md#status-markers-in-roadmaps).

Requirements have IDs (for example `FR-ACT-002`), as do properties (`P1`), scenarios (`S1`) and hazards (`H01`). Reference them in test names or doc comments so they can be traced.

ADRs record decisions that have been made. Do not reopen them silently. If you believe one is wrong, write a new ADR proposing a change and flag it.

Build in the milestone order in [spec/09-roadmap.md](spec/09-roadmap.md). Do not start the desktop UI (M4) until the core simulator properties (M2) pass. Tick roadmap items (`[~]`, `[x]`) in the same commit as the work.

## Core invariants (hard rules)

- No telemetry, analytics or crash reporting that leaves the practice. No calls to third-party services.
- Never show a green or "connected" status unless the device can currently deliver an alarm to at least one other device (`FR-STS-003`, P8).
- The sender's "Delivered to N" count only ever comes from real Receipts (P10, H03).
- Raising an alarm must never make an alarm-like sound on the sender's device (ADR 0008).
- Pointer movement never cancels a hold (ADR 0009).
- `muster-core` performs no I/O, reads no clocks and uses only injected randomness (ADR 0002).
- Decoding code must never panic on untrusted input. It is fuzzed.
- No patient data is ever handled, stored or transmitted.
- Do not copy the visual design, colours, iconography, sounds or wording of any commercial panic alarm product. Generate sounds programmatically and release them under CC0.
- Do not implement cryptographic primitives. Use the audited crates named in `spec/04-security.md`.
- Never log key material, enrolment tokens or message plaintext beyond the type and id.

## Toolchain and conventions

- Rust edition 2024, toolchain pinned in `rust-toolchain.toml`. Cargo workspace at the repo root.
- `#![forbid(unsafe_code)]` in `muster-proto`, `muster-crypto`, `muster-core`, `muster-sim` and `muster-relay`. Platform FFI (audio endpoint, DPAPI, session detection) lives in clearly named `platform` modules in `muster-agent` and `apps/desktop`, with `// SAFETY:` comments.
- Libraries use `thiserror`. Binaries use `anyhow`. Logging uses `tracing`. CLIs use `clap` derive and follow house-style [rust-cli.md](https://github.com/pacharanero/house-style/blob/main/rust-cli.md).
- Frontend (M4 onwards): TypeScript (strict), Svelte 5, Vite in `apps/desktop`. `eslint`, `prettier --check` and `svelte-check` must pass.
- Dependencies must be AGPL-compatible. `cargo-deny` (licences, advisories, bans, sources) and `cargo audit` enforce this. Prefer well-maintained crates and check them against house-style [dependencies.md](https://github.com/pacharanero/house-style/blob/main/dependencies.md).
- Every source file starts with the SPDX header:
  ```
  // SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
  // SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-Muster-app-store-exception
  ```
  Files that cannot carry a header are covered by `REUSE.toml`. `reuse lint` must pass.
- British English in UI strings and docs. Hyphens, not em dashes. Markdown is not hard-wrapped.

## Documented house-style exceptions

- **Desktop frontend:** Svelte 5 in `apps/desktop` (ADR 0003), not React + Mantine in `gui/`. Confirmed by Marcus Baw, 2026-11-14.
- **Supply-chain checks:** `cargo-deny` runs in addition to house-style `cargo audit`, because the spec requires licence and ban enforcement.
- **Clippy:** runs with `--all-features` as well as the house-style default.

## Workflow

- `s/test` - full test suite (unit, simulator scenarios and properties).
- `s/lint` - `cargo fmt --check`, clippy, `cargo deny check` (if installed), `reuse lint` (if installed).
- `s/fuzz <target>` - run a fuzz target (needs nightly and `cargo-fuzz`).
- See [s/README.md](s/README.md).

## Testing rules

- Every new piece of functionality comes with tests in the same commit.
- Protocol and state-machine behaviour is tested in the deterministic simulator (`muster-sim`), including the properties in `spec/07-testing.md`.
- A failing property test is a release blocker. Do not weaken a property to make it pass. Fix the code, or write an ADR explaining why the property was wrong.
- Agent-written tests are not sufficient evidence for cryptography or safety-critical behaviour on their own. Prefer independent vectors (RFC test vectors, golden files reviewed by a human) where they exist, and flag what still needs human review.

## Before every commit

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

Plus `cargo deny check` and `reuse lint` where installed. CI enforces all of these. Do not commit red.

## Git workflow

- Conventional Commits (`feat(core): ...`, `fix(net): ...`, `docs(spec): ...`). Mention roadmap codes where relevant (`implements R12`).
- Sign off every commit (`git commit -s`) per the DCO in `CONTRIBUTING.md`.
- Commit each validated coherent parcel without waiting to be asked.
- **Push policy:** while the repository has one active maintainer (Solo tier), agents may commit and push directly to `main` on `ohtf/muster`. Once a second regular contributor joins, switch to milestone branches `m<milestone>/<short-topic>` (for example `m2/alarm-state-crdt`) and PRs, one per milestone task where practical.
- PR descriptions: a two-or-three-line summary, then detail folded in `<details><summary>` (house-style [commits.md](https://github.com/pacharanero/house-style/blob/main/commits.md#pull-request-descriptions)).

## When the spec is ambiguous

Choose the safer option, implement it, record it as a short ADR (`spec/adr/NNNN-title.md`) and mention it in the commit or PR description. The safer option is the one that is more likely to deliver an alarm and less likely to mislead a user about whether it was delivered.

## Assurance

- Follow the house-style [class-wide fix directive](https://github.com/pacharanero/house-style/blob/main/agents.md#fix-the-class-not-just-the-instance).
- If behaviour that affects safety changes, update `safety/hazard-log.md` in the same change.

## Fetch, do not invent

- Licence texts are fetched, not typed (`LICENSE` from gnu.org; `CODE_OF_CONDUCT.md` from contributor-covenant.org).
- Check the default UDP/TCP port (47474) against the IANA registry before the first release (R50).
- Pin GitHub Actions to full commit SHAs looked up at the time, never from memory.

## Approval required

Ask before publishing releases, creating tags, deleting branches, force-pushing, changing repository settings or secrets, making the repository public, or posting issues or comments.
