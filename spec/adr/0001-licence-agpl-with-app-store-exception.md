# ADR 0001: AGPL-3.0-or-later with an app store exception, and DCO

- Status: Accepted (2026-11-14)
- Decider: Marcus (OHTF)

## Context

The project must stay open and must not be taken closed-source and resold, which is exactly what it is trying to replace. The relay runs as a network service, so plain GPL would not cover modified hosted versions. Mobile distribution through the Apple App Store is widely considered incompatible with GPL-family licences unless the copyright holders grant an additional permission.

## Decision

- Licence: AGPL-3.0-or-later.
- Add an additional permission under section 7 allowing app store distribution, provided source is available elsewhere (`LICENSE-EXCEPTIONS.md`). The draft text must be reviewed before the first external contribution.
- Contributions use the Developer Certificate of Origin (sign-off), not a CLA. Revenue comes from hardware and services, not dual licensing, so copyright assignment is unnecessary.
- Trade marks (name, logo, OHTF branding) are protected separately by a trade mark policy.
- Hardware: CERN-OHL-S-2.0.

## Consequences

- The exception must be in place from the first commit. Adding it later needs every contributor's agreement.
- Dependencies must be AGPL-compatible (enforced by cargo-deny).
- The relay must offer source to its users (`/source`).
