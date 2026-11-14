# Open queries

Questions that need a human decision. When one is answered, record the answer (and an ADR if it changes a decision) and mark it resolved.

Legend: `[ ]` open, `[x]` resolved.

- [x] **Q1 - Repository layout.** The original spec placed the specification in `docs/spec/`, ADRs in `docs/adr/` and the hazard log in `docs/safety/`. Resolved 2026-11-14 (Marcus Baw): specification in `spec/`, ADRs in `spec/adr/`, hazard log in `safety/` with a root `SAFETY.md`, per house style. `docs/` is reserved for user-facing guides and testing records. See ADR 0011.
- [x] **Q2 - Desktop frontend stack.** House style defaults to React + Mantine in `gui/`; ADR 0003 chose Svelte 5 in `apps/desktop`. Resolved 2026-11-14 (Marcus Baw): keep ADR 0003. Recorded as a documented house-style exception in `agent-instructions.md`.
- [x] **Q3 - Copyright holder.** Resolved 2026-11-14 (Marcus Baw): `Open Health Technology Foundation contributors`.
- [x] **Q4 - Documentation licence.** Resolved 2026-11-14 (Marcus Baw): CC-BY-SA-4.0.
- [ ] **Q5 - Medical-device applicability.** The assessment in `SAFETY.md` needs a named decision-maker, conclusion and review date.
- [ ] **Q6 - OHTF palette for the desktop app.** The spec says "OHTF palette (to be supplied; until then deep teal `#0E5E6F`)". The OHTF website now defines Surgical Green `#0D4A3B` with a white-mixed tint scale. Confirm whether the desktop app adopts it.
- [ ] **Q7 - Windows control API transport.** The spec says a named pipe `\\.\pipe\muster-<SID>` with an ACL for the current user and SYSTEM. v1 currently serves the control API on Unix sockets only; Windows needs the named pipe and its security descriptor (M4, platform module).
- [ ] **Q8 - Single-device status after creating a token.** A practice whose only device has issued an enrolment token shows red "No other devices reachable" rather than amber "No other devices found", because the enroller cert counts as evidence that other devices exist. This errs towards honesty; confirm it is the wording wanted.
