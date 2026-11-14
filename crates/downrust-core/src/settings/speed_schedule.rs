// SPDX-License-Identifier: GPL-3.0-or-later
//! Speed limits that change with the time of day and the day of the week.
use chrono::{Datelike, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};

use super::schedule::{in_window, Schedule};

/// Lowest limit a rule may set, in bytes per second.
pub const MIN_RULE_LIMIT: u64 = 1024;

/// One time window with its own speed limit. Days run Monday (0) to Sunday (6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeedRule {
    pub days: Vec<u8>,
    /// Local time as "HH:MM".
    pub start: String,
    /// Local time as "HH:MM". An end before the start runs over midnight.
    pub end: String,
    /// Bytes per second. `None` means no limit.
    pub limit: Option<u64>,
}

/// Speed rules and the switch that turns them on. The first enabled matching rule wins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeedSchedule {
    pub enabled: bool,
    pub rules: Vec<SpeedRule>,
}

/// Parses "HH:MM" into minutes since midnight.
fn parse_minute(text: &str) -> Option<u16> {
    let (hours, minutes) = text.split_once(':')?;
    let hours: u16 = hours.parse().ok()?;
    let minutes: u16 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

impl SpeedRule {
    /// The rule as a time window. `None` when a time is invalid.
    fn window(&self) -> Option<Schedule> {
        let start = parse_minute(&self.start)?;
        let stop = parse_minute(&self.end)?;
        let mut days = [false; 7];
        for day in self.days.iter().filter(|day| **day < 7) {
            days[usize::from(*day)] = true;
        }
        Some(Schedule {
            enabled: true,
            start_minute: start,
            stop_minute: stop,
            days,
        })
    }
}

impl SpeedSchedule {
    /// Speed limit that applies at `now`. See [`effective_limit`].
    pub fn limit_at(&self, base: Option<u64>, now: NaiveDateTime) -> Option<u64> {
        effective_limit(self, base, now)
    }

    /// Drops rules with invalid times, drops invalid days and raises low limits to the minimum.
    /// A limit of `Some(0)` becomes `None`, which means no limit.
    pub fn sanitized(mut self) -> Self {
        self.rules.retain(|rule| rule.window().is_some());
        for rule in &mut self.rules {
            rule.days.retain(|day| *day < 7);
            rule.days.sort_unstable();
            rule.days.dedup();
            rule.limit = match rule.limit {
                None | Some(0) => None,
                Some(limit) => Some(limit.max(MIN_RULE_LIMIT)),
            };
        }
        self
    }
}

/// Speed limit that applies at `now`, in bytes per second. `None` means no limit.
///
/// A disabled schedule returns `base`, the global limit. Otherwise the first matching rule's limit
/// applies. When no rule matches, `base` applies.
pub fn effective_limit(
    schedule: &SpeedSchedule,
    base: Option<u64>,
    now: NaiveDateTime,
) -> Option<u64> {
    if !schedule.enabled {
        return base;
    }
    let weekday = now.weekday().num_days_from_monday();
    let minute = u16::try_from(now.hour() * 60 + now.minute()).unwrap_or(0);
    for rule in &schedule.rules {
        let Some(window) = rule.window() else {
            continue;
        };
        if in_window(&window, weekday, minute) {
            return rule.limit;
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// 2026-11-16 is a Monday.
    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, day)
            .expect("date")
            .and_hms_opt(hour, minute, 0)
            .expect("time")
    }

    fn rule(days: &[u8], start: &str, end: &str, limit: Option<u64>) -> SpeedRule {
        SpeedRule {
            days: days.to_vec(),
            start: start.to_owned(),
            end: end.to_owned(),
            limit,
        }
    }

    fn schedule(rules: Vec<SpeedRule>) -> SpeedSchedule {
        SpeedSchedule {
            enabled: true,
            rules,
        }
    }

    #[test]
    fn disabled_schedule_returns_the_base_limit() {
        let s = SpeedSchedule {
            enabled: false,
            rules: vec![rule(&[0], "00:00", "23:59", Some(4096))],
        };
        assert_eq!(effective_limit(&s, Some(9000), at(16, 12, 0)), Some(9000));
        assert_eq!(effective_limit(&s, None, at(16, 12, 0)), None);
    }

    #[test]
    fn matching_rule_limit_wins_over_base() {
        let s = schedule(vec![rule(&[0, 1], "09:00", "17:00", Some(2048))]);
        assert_eq!(effective_limit(&s, Some(9000), at(16, 10, 0)), Some(2048));
        assert_eq!(effective_limit(&s, Some(9000), at(16, 17, 0)), Some(9000));
    }

    #[test]
    fn rule_with_no_limit_means_unlimited() {
        let s = schedule(vec![rule(&[0], "00:00", "23:59", None)]);
        assert_eq!(effective_limit(&s, Some(9000), at(16, 8, 0)), None);
    }

    #[test]
    fn day_boundaries_are_respected() {
        let s = schedule(vec![rule(&[1], "10:00", "11:00", Some(4096))]);
        // 2026-11-17 is Tuesday (index 1); 2026-11-16 is Monday (index 0).
        assert_eq!(effective_limit(&s, None, at(17, 10, 30)), Some(4096));
        assert_eq!(effective_limit(&s, None, at(16, 10, 30)), None);
        assert_eq!(effective_limit(&s, None, at(17, 11, 0)), None);
    }

    #[test]
    fn rule_over_midnight_covers_both_sides() {
        let s = schedule(vec![rule(&[0], "22:00", "06:00", Some(4096))]);
        // Monday night belongs to the Monday rule.
        assert_eq!(effective_limit(&s, None, at(16, 23, 0)), Some(4096));
        // Tuesday 05:00 belongs to Monday's rule.
        assert_eq!(effective_limit(&s, None, at(17, 5, 0)), Some(4096));
        // Tuesday 07:00 is outside it.
        assert_eq!(effective_limit(&s, None, at(17, 7, 0)), None);
    }

    #[test]
    fn first_matching_rule_wins() {
        let s = schedule(vec![
            rule(&[0], "09:00", "12:00", Some(2048)),
            rule(&[0], "00:00", "23:59", Some(8192)),
        ]);
        assert_eq!(effective_limit(&s, None, at(16, 10, 0)), Some(2048));
        assert_eq!(effective_limit(&s, None, at(16, 13, 0)), Some(8192));
    }

    #[test]
    fn invalid_rules_are_ignored_at_match_time() {
        let s = schedule(vec![rule(&[0], "25:00", "12:00", Some(2048))]);
        assert_eq!(effective_limit(&s, Some(9000), at(16, 10, 0)), Some(9000));
    }

    #[test]
    fn sanitize_drops_bad_times_and_clamps_limits() {
        let s = schedule(vec![
            rule(&[0, 9, 0], "08:00", "09:00", Some(10)),
            rule(&[1], "8am", "09:00", Some(4096)),
            rule(&[2], "08:00", "09:00", Some(0)),
        ])
        .sanitized();
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.rules[0].days, vec![0]);
        assert_eq!(s.rules[0].limit, Some(MIN_RULE_LIMIT));
        assert_eq!(s.rules[1].limit, None);
    }

    #[test]
    fn default_is_disabled_and_empty() {
        let s = SpeedSchedule::default();
        assert!(!s.enabled);
        assert!(s.rules.is_empty());
    }
}
