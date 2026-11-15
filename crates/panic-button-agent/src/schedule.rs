// SPDX-FileCopyrightText: 2026 Open Health Technology Foundation contributors
// SPDX-License-Identifier: AGPL-3.0-or-later WITH LicenseRef-PanicButton-app-store-exception

//! Weekly drills (FR-DRL-001, FR-DRL-006). The practice setting
//! `drill_schedule` is a day and a local time, for example `Tue 07:45`.
//! The coordinator device runs the drill; admin devices record it as
//! skipped if no drill from the coordinator was seen around that time.
//!
//! Local time lives here rather than in the core, which only knows UTC.

use jiff::civil::{Time, Weekday};
use jiff::{ToSpan, Zoned};

/// A parsed weekly schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weekly {
    pub day: Weekday,
    pub time: Time,
}

/// How long after the slot a drill may still start (the agent may have
/// been asleep or busy at the minute itself).
pub const RUN_WINDOW_MS: u64 = 10 * 60 * 1000;
/// Admins wait this long after the slot before recording a skip.
pub const SKIP_AFTER_MS: u64 = 15 * 60 * 1000;

/// Parses `Tue 07:45` (day names in English, three letters or full; case
/// does not matter).
pub fn parse(s: &str) -> Result<Weekly, String> {
    let mut it = s.split_whitespace();
    let (Some(d), Some(t), None) = (it.next(), it.next(), it.next()) else {
        return Err(format!(
            "{s:?}: expected a day and a time, like \"Tue 07:45\""
        ));
    };
    let day = match d.to_ascii_lowercase().get(..3) {
        Some("mon") => Weekday::Monday,
        Some("tue") => Weekday::Tuesday,
        Some("wed") => Weekday::Wednesday,
        Some("thu") => Weekday::Thursday,
        Some("fri") => Weekday::Friday,
        Some("sat") => Weekday::Saturday,
        Some("sun") => Weekday::Sunday,
        _ => return Err(format!("{d:?} is not a day of the week")),
    };
    let (h, m) = t
        .split_once(':')
        .ok_or_else(|| format!("{t:?}: expected a time like 07:45"))?;
    let (h, m): (i8, i8) = (
        h.parse().map_err(|_| format!("{t:?} is not a time"))?,
        m.parse().map_err(|_| format!("{t:?} is not a time"))?,
    );
    let time = Time::new(h, m, 0, 0).map_err(|_| format!("{t:?} is not a time"))?;
    Ok(Weekly { day, time })
}

/// The most recent slot at or before `now`, in Unix ms.
pub fn last_slot(w: Weekly, now: &Zoned) -> Option<u64> {
    let today = now.date();
    let back = (i64::from(today.weekday().to_monday_zero_offset())
        - i64::from(w.day.to_monday_zero_offset()))
    .rem_euclid(7);
    let mut date = today.checked_sub(back.days()).ok()?;
    loop {
        let slot = date
            .to_datetime(w.time)
            .to_zoned(now.time_zone().clone())
            .ok()?;
        if slot.timestamp() <= now.timestamp() {
            return u64::try_from(slot.timestamp().as_millisecond()).ok();
        }
        date = date.checked_sub(7.days()).ok()?;
    }
}

/// What the scheduler should do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    Nothing,
    /// This device is the coordinator: run the drill for this slot.
    Run(u64),
    /// No drill was seen for this slot: record it as skipped.
    Skipped(u64),
}

/// `ran` says whether a drill from the coordinator started within the run
/// window of `slot` (and, on the coordinator, whether this device already
/// ran one); `recorded` whether a skip for this slot is already recorded.
pub fn due(
    slot: u64,
    now_ms: u64,
    coordinator: bool,
    admin: bool,
    ran: bool,
    recorded: bool,
) -> Due {
    let since = now_ms.saturating_sub(slot);
    if ran {
        Due::Nothing
    } else if coordinator && since <= RUN_WINDOW_MS {
        Due::Run(slot)
    } else if admin && since >= SKIP_AFTER_MS && !recorded {
        Due::Skipped(slot)
    } else {
        Due::Nothing
    }
}

/// A drill started at `started` counts for `slot`. Scheduled drills start
/// at or after the slot; 30 s allows for clocks that differ slightly.
pub fn counts_for(slot: u64, started: u64) -> bool {
    started + 30_000 >= slot && started <= slot + RUN_WINDOW_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoned(s: &str) -> Zoned {
        s.parse().expect("zoned")
    }

    #[test]
    fn parses() {
        let w = parse("tue 07:45").expect("ok");
        assert_eq!(
            (w.day, w.time),
            (Weekday::Tuesday, Time::constant(7, 45, 0, 0))
        );
        assert_eq!(
            parse("Wednesday 18:00").expect("ok").day,
            Weekday::Wednesday
        );
        for bad in [
            "",
            "Tue",
            "Tue 25:00",
            "Xyz 07:45",
            "Tue 07:45 extra",
            "Tue 0745",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn last_slot_in_local_time() {
        let w = parse("Tue 07:45").expect("ok");
        // Tuesday 2026-11-17 08:00 in London (GMT): slot 07:45 the same day.
        let now = zoned("2026-11-17T08:00[Europe/London]");
        let s = last_slot(w, &now).expect("slot");
        assert_eq!(
            s,
            zoned("2026-11-17T07:45[Europe/London]")
                .timestamp()
                .as_millisecond() as u64
        );
        // Tuesday 07:30: the previous week's slot.
        let now = zoned("2026-11-17T07:30[Europe/London]");
        let s = last_slot(w, &now).expect("slot");
        assert_eq!(
            s,
            zoned("2026-11-10T07:45[Europe/London]")
                .timestamp()
                .as_millisecond() as u64
        );
        // Summer time is local time too.
        let now = zoned("2026-06-02T07:50[Europe/London]");
        let s = last_slot(w, &now).expect("slot");
        assert_eq!(
            s,
            zoned("2026-06-02T06:45Z[UTC]").timestamp().as_millisecond() as u64
        );
    }

    #[test]
    fn coordinator_runs_and_admins_record_skips() {
        let slot = 1_000_000_000;
        assert_eq!(
            due(slot, slot + 1000, true, true, false, false),
            Due::Run(slot)
        );
        assert_eq!(
            due(slot, slot + 1000, true, true, true, false),
            Due::Nothing
        );
        assert_eq!(
            due(slot, slot + RUN_WINDOW_MS + 1, true, false, false, false),
            Due::Nothing
        );
        assert_eq!(
            due(slot, slot + 1000, false, true, false, false),
            Due::Nothing
        );
        assert_eq!(
            due(slot, slot + SKIP_AFTER_MS, false, true, false, false),
            Due::Skipped(slot)
        );
        assert_eq!(
            due(slot, slot + SKIP_AFTER_MS, false, true, false, true),
            Due::Nothing
        );
        assert_eq!(
            due(slot, slot + SKIP_AFTER_MS, false, false, false, false),
            Due::Nothing
        );
        assert!(counts_for(slot, slot + 5000));
        assert!(!counts_for(slot, slot - 10 * 60 * 1000));
        assert!(
            !counts_for(slot, slot - 60_000),
            "a manual drill just before"
        );
    }
}
