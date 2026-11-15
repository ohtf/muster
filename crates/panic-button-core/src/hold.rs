// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! The hold machine (`spec/05-desktop-agent.md`, Hold interaction).
//!
//! ```text
//! Idle --HoldStart(src)--> Holding{start, src}
//! Holding --HoldEnd, elapsed < 300ms--> Idle                 (nothing logged)
//! Holding --HoldEnd, 300ms <= elapsed < hold_ms--> Idle      (NearMiss)
//! Holding --Tick, elapsed >= hold_ms--> Triggered --> Idle
//! ```
//!
//! Timing is decided here, not in the UI, so every source behaves the same
//! (FR-ACT-008). Pointer movement is not an input at all (ADR 0009).

use panic_button_proto::Source;

use crate::time::{HOLD_PROGRESS_INTERVAL, Mono, NEAR_MISS_MIN_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldState {
    Idle,
    Holding {
        start: Mono,
        source: Source,
        practice: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldOutcome {
    /// Nothing to report.
    None,
    /// A hold started; start the cue.
    Started,
    /// Released before the near-miss threshold: stop the cue, log nothing.
    Released,
    /// Released between 300 ms and `hold_ms` (FR-ACT-009).
    NearMiss { source: Source, duration_ms: u64 },
    /// Held for `hold_ms`: raise the alarm (unless `practice`).
    Triggered { source: Source, practice: bool },
}

#[derive(Debug, Clone)]
pub struct HoldMachine {
    state: HoldState,
    last_progress: Mono,
}

impl Default for HoldMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl HoldMachine {
    pub fn new() -> Self {
        HoldMachine {
            state: HoldState::Idle,
            last_progress: Mono::ZERO,
        }
    }

    pub fn state(&self) -> HoldState {
        self.state
    }

    /// Only one hold can be active; a second source is ignored.
    pub fn start(&mut self, now: Mono, source: Source, practice: bool) -> HoldOutcome {
        match self.state {
            HoldState::Idle => {
                self.state = HoldState::Holding {
                    start: now,
                    source,
                    practice,
                };
                self.last_progress = now;
                HoldOutcome::Started
            }
            HoldState::Holding { .. } => HoldOutcome::None,
        }
    }

    /// A release from a source other than the one holding is ignored.
    pub fn end(&mut self, now: Mono, source: Source, hold_ms: u32) -> HoldOutcome {
        let HoldState::Holding {
            start,
            source: s,
            practice,
        } = self.state
        else {
            return HoldOutcome::None;
        };
        if s != source {
            return HoldOutcome::None;
        }
        let elapsed = now.since(start);
        if elapsed >= u64::from(hold_ms) {
            // The tick that should have triggered has not been delivered yet;
            // treat the hold as complete rather than lose it.
            self.state = HoldState::Idle;
            return HoldOutcome::Triggered { source, practice };
        }
        self.state = HoldState::Idle;
        if elapsed >= NEAR_MISS_MIN_MS {
            HoldOutcome::NearMiss {
                source,
                duration_ms: elapsed,
            }
        } else {
            HoldOutcome::Released
        }
    }

    /// Advances time. Returns `Triggered` once the hold reaches `hold_ms`.
    pub fn tick(&mut self, now: Mono, hold_ms: u32) -> HoldOutcome {
        let HoldState::Holding {
            start,
            source,
            practice,
        } = self.state
        else {
            return HoldOutcome::None;
        };
        if now.since(start) >= u64::from(hold_ms) {
            self.state = HoldState::Idle;
            return HoldOutcome::Triggered { source, practice };
        }
        HoldOutcome::None
    }

    /// Progress in thousandths, if a progress event is due.
    pub fn progress_due(&mut self, now: Mono, hold_ms: u32) -> Option<u16> {
        let HoldState::Holding { start, .. } = self.state else {
            return None;
        };
        if now.since(self.last_progress) < HOLD_PROGRESS_INTERVAL && now != start {
            return None;
        }
        self.last_progress = now;
        let f = (now.since(start) * 1000 / u64::from(hold_ms.max(1))).min(1000);
        Some(f as u16)
    }

    pub fn deadline(&self, hold_ms: u32) -> Option<Mono> {
        match self.state {
            HoldState::Idle => None,
            HoldState::Holding { start, .. } => {
                Some((self.last_progress + HOLD_PROGRESS_INTERVAL).min(start + u64::from(hold_ms)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLD: u32 = 1500;

    fn released_at(ms: u64) -> HoldOutcome {
        let mut h = HoldMachine::new();
        assert_eq!(
            h.start(Mono(1000), Source::Button, false),
            HoldOutcome::Started
        );
        assert_eq!(
            h.tick(Mono(1000 + ms.saturating_sub(1)), HOLD),
            HoldOutcome::None
        );
        h.end(Mono(1000 + ms), Source::Button, HOLD)
    }

    #[test]
    fn fr_act_009_near_miss_boundaries() {
        assert_eq!(released_at(299), HoldOutcome::Released);
        assert_eq!(
            released_at(300),
            HoldOutcome::NearMiss {
                source: Source::Button,
                duration_ms: 300
            }
        );
        assert_eq!(
            released_at(1499),
            HoldOutcome::NearMiss {
                source: Source::Button,
                duration_ms: 1499
            }
        );
    }

    #[test]
    fn fr_act_002_triggers_at_hold_ms() {
        let mut h = HoldMachine::new();
        h.start(Mono(0), Source::Hotkey, false);
        assert_eq!(h.tick(Mono(1499), HOLD), HoldOutcome::None);
        assert_eq!(
            h.tick(Mono(1500), HOLD),
            HoldOutcome::Triggered {
                source: Source::Hotkey,
                practice: false
            }
        );
        assert_eq!(h.state(), HoldState::Idle);
        // The release after triggering does nothing.
        assert_eq!(h.end(Mono(1600), Source::Hotkey, HOLD), HoldOutcome::None);
    }

    #[test]
    fn late_release_after_hold_ms_still_triggers() {
        let mut h = HoldMachine::new();
        h.start(Mono(0), Source::Button, false);
        assert!(matches!(
            h.end(Mono(1500), Source::Button, HOLD),
            HoldOutcome::Triggered { .. }
        ));
    }

    #[test]
    fn second_source_is_ignored() {
        let mut h = HoldMachine::new();
        h.start(Mono(0), Source::Button, false);
        assert_eq!(h.start(Mono(100), Source::Hotkey, false), HoldOutcome::None);
        assert_eq!(h.end(Mono(200), Source::Hotkey, HOLD), HoldOutcome::None);
        assert!(matches!(
            h.state(),
            HoldState::Holding {
                source: Source::Button,
                ..
            }
        ));
    }

    #[test]
    fn progress_is_rate_limited_and_reaches_full() {
        let mut h = HoldMachine::new();
        h.start(Mono(0), Source::Button, false);
        assert_eq!(h.progress_due(Mono(0), HOLD), Some(0));
        assert_eq!(h.progress_due(Mono(10), HOLD), None);
        assert_eq!(h.progress_due(Mono(750), HOLD), Some(500));
        assert_eq!(h.progress_due(Mono(1600), HOLD), Some(1000));
        assert_eq!(h.deadline(HOLD), Some(Mono(1500)));
    }
}
