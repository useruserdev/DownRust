// SPDX-License-Identifier: GPL-3.0-or-later
//! Time window in which downloads may run.
use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};

/// Minutes in one day. Valid minute values are 0 to 1439.
const LAST_MINUTE: u16 = 1439;

/// Window of the day in which downloads may run. Days run Monday (index 0) to Sunday (index 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Schedule {
    pub enabled: bool,
    pub start_minute: u16, // minutes since local midnight, 0..=1439
    pub stop_minute: u16,  // minutes since local midnight, 0..=1439
    pub days: [bool; 7],   // Monday..Sunday
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            enabled: false,
            start_minute: 120,
            stop_minute: 480,
            days: [true; 7],
        }
    }
}

impl Schedule {
    /// Clamps both minutes to the valid range of one day.
    pub fn sanitized(mut self) -> Self {
        self.start_minute = self.start_minute.min(LAST_MINUTE);
        self.stop_minute = self.stop_minute.min(LAST_MINUTE);
        self
    }
}

/// Reports whether downloads may run at the given local weekday (0 = Monday) and minute of day.
///
/// A disabled schedule never restricts. A window whose start equals its stop covers the whole day.
/// A window that crosses midnight belongs to the day on which it starts. Its part after midnight
/// is allowed when the previous day is selected.
pub fn in_window(s: &Schedule, weekday_from_monday: u32, minute: u16) -> bool {
    if !s.enabled {
        return true;
    }
    let day = usize::try_from(weekday_from_monday % 7).unwrap_or(0);
    let previous = (day + 6) % 7;
    if s.start_minute == s.stop_minute {
        s.days[day]
    } else if s.start_minute < s.stop_minute {
        s.days[day] && minute >= s.start_minute && minute < s.stop_minute
    } else {
        (s.days[day] && minute >= s.start_minute) || (s.days[previous] && minute < s.stop_minute)
    }
}

/// Reports whether downloads may run at this moment, using the local clock.
pub fn is_open_now(s: &Schedule) -> bool {
    let now = Local::now();
    let minute = u16::try_from(now.hour() * 60 + now.minute()).unwrap_or(0);
    in_window(s, now.weekday().num_days_from_monday(), minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(start: u16, stop: u16, days: [bool; 7]) -> Schedule {
        Schedule {
            enabled: true,
            start_minute: start,
            stop_minute: stop,
            days,
        }
    }

    const ALL: [bool; 7] = [true; 7];

    #[test]
    fn disabled_schedule_never_restricts() {
        let s = Schedule {
            enabled: false,
            start_minute: 600,
            stop_minute: 700,
            days: [false; 7],
        };
        assert!(in_window(&s, 3, 0));
        assert!(in_window(&s, 3, 1439));
    }

    #[test]
    fn same_start_and_stop_covers_the_whole_day() {
        let s = window(300, 300, ALL);
        assert!(in_window(&s, 0, 0));
        assert!(in_window(&s, 6, 1439));
        let only_monday = window(300, 300, [true, false, false, false, false, false, false]);
        assert!(!in_window(&only_monday, 1, 500));
    }

    #[test]
    fn daytime_window_includes_start_and_excludes_stop() {
        let s = window(120, 480, ALL);
        assert!(!in_window(&s, 2, 119));
        assert!(in_window(&s, 2, 120));
        assert!(in_window(&s, 2, 479));
        assert!(!in_window(&s, 2, 480));
    }

    #[test]
    fn daytime_window_respects_selected_days() {
        let s = window(120, 480, [true, false, true, false, true, false, false]);
        assert!(in_window(&s, 0, 200));
        assert!(!in_window(&s, 1, 200));
        assert!(in_window(&s, 2, 200));
        assert!(!in_window(&s, 6, 200));
    }

    #[test]
    fn midnight_window_before_start_is_closed() {
        let s = window(1380, 60, ALL);
        assert!(!in_window(&s, 4, 1379));
    }

    #[test]
    fn midnight_window_after_start_is_open_on_its_day() {
        let s = window(1380, 60, ALL);
        assert!(in_window(&s, 4, 1380));
        assert!(in_window(&s, 4, 1439));
    }

    #[test]
    fn midnight_window_before_stop_uses_the_previous_day() {
        let s = window(1380, 60, [true, false, false, false, false, false, false]);
        // Tuesday 00:30 belongs to Monday's window.
        assert!(in_window(&s, 1, 30));
        // Monday 00:30 belongs to Sunday's window, which is not selected.
        assert!(!in_window(&s, 0, 30));
    }

    #[test]
    fn midnight_window_stop_is_exclusive() {
        let s = window(1380, 60, ALL);
        assert!(in_window(&s, 2, 59));
        assert!(!in_window(&s, 2, 60));
        assert!(!in_window(&s, 2, 1000));
    }

    #[test]
    fn sanitized_clamps_minutes_to_one_day() {
        let s = Schedule {
            start_minute: 5000,
            stop_minute: 1440,
            ..Schedule::default()
        }
        .sanitized();
        assert_eq!(s.start_minute, 1439);
        assert_eq!(s.stop_minute, 1439);
    }

    #[test]
    fn default_is_disabled_two_to_eight() {
        let s = Schedule::default();
        assert!(!s.enabled);
        assert_eq!((s.start_minute, s.stop_minute), (120, 480));
        assert_eq!(s.days, ALL);
    }
}
