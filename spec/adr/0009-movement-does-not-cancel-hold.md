# ADR 0009: Pointer movement never cancels a hold

- Status: Accepted (2026-11-14)

## Context

The floating button must be movable. The obvious pattern ("drag if the pointer moves more than N px") would cancel genuine alarms when a frightened user's hand moves during the hold.

## Decision

- A hold ends only on release, pointer cancel or lost capture. Movement is ignored.
- Repositioning requires an explicit mode: tray "Move button" (30 s), or Shift held at pointer down.

## Consequences

- Slightly less discoverable repositioning. The first-run flow and tooltip mention it.
