# ADR 0007: A single alarm kind, "come and help"

- Status: Accepted (2026-11-14)
- Decider: Marcus (OHTF), clinical judgement

## Context

Commercial and security products often offer several alarm types (attack, medical, fire and so on). In a GP practice, the response to any of these is the same first step: colleagues go to the room. Asking a person under stress to choose a type adds time and errors.

## Decision

There is one alarm, meaning "come and help". Responders judge what is needed on arrival. The protocol includes a `kind` field (value 1 = help). Receivers treat unknown kinds as help, so types can be added later without breaking older devices.

## Consequences

- Simpler UI and training.
- Responders get no advance information on the nature of the emergency. This is accepted.
