# ADR 0008: Hold to raise, soft cue during the hold, silent to the room afterwards

- Status: Accepted (2026-11-14)
- Decider: Marcus (OHTF)

## Context

Accidental activation causes alarm fatigue and leads staff to disable the software. Audible activation can tip off an aggressor: the danger is less that they stop the press within 1.5 s, and more that they know help has been called, which can escalate the situation. Repeated exposure would also teach regular patients what the alarm sounds like.

## Decision

- Press and hold for `hold_ms` (default 1500 ms) on the floating button or the shortcut. The core enforces the timing for all sources.
- During the hold: a filling ring, plus (default `cue_mode = soft`) a quiet, rising, non-alarm tone that only the person at the desk is likely to notice. Releasing stops it immediately. This catches accidental presses the user is not looking at (sleeve on keys, knee on a button).
- After triggering: **no sound** on the sender's device, in any mode. Only a small, neutral confirmation panel.
- `cue_mode` can be `silent`, `soft` or `alarm_ramp` (practice-configurable). `alarm_ramp` is the home profile default, where covertness is not a concern.
- Holds released between 300 ms and `hold_ms` are logged as near misses so the threshold and cue can be tuned with evidence.
- No countdown after triggering. Mistakes are handled by "Cancel - sent in error".

## Consequences

- Hardware buttons and mobile must use the same core hold machine.
- Lab test L10 and L11 measure accidental and deliberate activation.
