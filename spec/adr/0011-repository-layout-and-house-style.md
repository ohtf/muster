# ADR 0011: Repository layout and house-style alignment

- Status: Accepted (2026-11-14)
- Decider: Marcus (OHTF)

## Context

The original specification bundle placed the spec, ADRs and hazard log under `docs/`. OHTF projects follow the [pacharanero/house-style](https://github.com/pacharanero/house-style) standards, which put durable design in `spec/`, a clinical safety entry point in a root `SAFETY.md`, scripts in `s/`, and agent guidance in a vendor-neutral `agent-instructions.md`.

## Decision

- Specification: `spec/00-overview.md` to `spec/09-roadmap.md`, indexed by `spec/README.md`. Open questions in `spec/queries.md`.
- ADRs: `spec/adr/`.
- Clinical safety: root `SAFETY.md`, hazard log at `safety/hazard-log.md`.
- `docs/` holds user, admin, IT and relay operator guides, and hardware lab testing records (`docs/testing/results/`).
- Agent guidance: `agent-instructions.md`, with thin `AGENTS.md` and `CLAUDE.md` pointers.
- Repeated processes: `s/` scripts (`s/test`, `s/lint`, ...).
- Roadmap items carry checkboxes and stable `R<n>` codes.
- The desktop app stays in `apps/desktop` with Svelte 5 (ADR 0003), a documented exception to the house-style Tauri default of React + Mantine in `gui/`.

## Consequences

- Cross-references in the spec use the new paths.
- House-style tooling (REUSE, zizmor, cargo-audit, Dependabot cooldowns) applies in addition to the spec's own requirements (cargo-deny).
