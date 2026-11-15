# ADR 0010: Machine-scoped device identity on Windows

- Status: Accepted (2026-11-14)

## Context

GP practice PCs are shared and hot-desked by many users. A per-user identity would multiply devices (users x machines), each needing enrolment, and make coverage reports meaningless. The agent runs per user session (it needs the desktop for UI and sound).

## Decision

- On Windows, the device identity and practice keys are per machine, stored in `%ProgramData%\Panic Button\identity`, encrypted with DPAPI machine scope (`CRYPTPROTECT_LOCAL_MACHINE`), readable by local users so the per-session agent can load them.
- Only the agent in the active console session participates (FR-PLT-004).
- The heartbeat and alarms carry the current user's display name separately.
- macOS and Linux use per-user identity in v1 (single-user machines are the norm there). Revisit if shared Macs appear in practices.

## Consequences

- Any local user on the machine can, with effort, use the machine identity. This is accepted: they are practice staff on a practice machine.
- DPAPI machine scope stops the key file working if it is copied to another machine.
