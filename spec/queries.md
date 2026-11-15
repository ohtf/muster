# Open queries

Questions that need a human decision. When one is answered, record the answer (and an ADR if it changes a decision) and mark it resolved.

Legend: `[ ]` open, `[x]` resolved.

- [x] **Q1 - Repository layout.** The original spec placed the specification in `docs/spec/`, ADRs in `docs/adr/` and the hazard log in `docs/safety/`. Resolved 2026-11-14 (Marcus Baw): specification in `spec/`, ADRs in `spec/adr/`, hazard log in `safety/` with a root `SAFETY.md`, per house style. `docs/` is reserved for user-facing guides and testing records. See ADR 0011.
- [x] **Q2 - Desktop frontend stack.** House style defaults to React + Mantine in `gui/`; ADR 0003 chose Svelte 5 in `apps/desktop`. Resolved 2026-11-14 (Marcus Baw): keep ADR 0003. Recorded as a documented house-style exception in `agent-instructions.md`.
- [x] **Q3 - Copyright holder.** Resolved 2026-11-14 (Marcus Baw): `Open Health Technology Foundation contributors`.
- [x] **Q4 - Documentation licence.** Resolved 2026-11-14 (Marcus Baw): CC-BY-SA-4.0.
- [x] **Q5 - Medical-device applicability.** Resolved 2026-11-14 (Marcus Baw): not a medical device; it is standard office equipment. Recorded in `SAFETY.md` with a review date.
- [x] **Q6 - Palette for the desktop app.** Resolved 2026-11-14 (Marcus Baw): not the OHTF palette; a palette that suits the product, built around a big red button. Defined in `spec/05-desktop-agent.md`, Visual identity.
- [x] **Q7 - Windows control API transport.** Resolved 2026-11-14 (Marcus Baw): implement the named pipe as specified, in R32. Implemented as `\\.\pipe\panic-button-<domain>-<user>` with Windows' default pipe security (full access for the owner, SYSTEM and Administrators; read-only for others, which cannot send requests), remote clients refused, and `first_pipe_instance` so the name cannot be squatted. This avoids unsafe Win32 calls to look up the SID and build a custom security descriptor; tighten it to the user's SID and SYSTEM only if Administrators' access is a concern.
- [x] **Q8 - Single-device status after creating a token.** Resolved 2026-11-14 (Marcus Baw): keep the current behaviour and settle the wording in user testing (R33). A practice whose only device has issued an enrolment token shows red "No other devices reachable" rather than amber "No other devices found", because the enroller cert counts as evidence that other devices exist. This errs towards honesty; confirm it is the wording wanted.
