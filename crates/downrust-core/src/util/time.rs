// SPDX-License-Identifier: GPL-3.0-or-later
//! Time helpers.
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current unix time in milliseconds.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// Local calendar date (`YYYY-MM-DD`) of a unix time in milliseconds.
pub fn local_day(ms: i64) -> Option<String> {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|moment| moment.format("%Y-%m-%d").to_string())
}

/// Local calendar date (`YYYY-MM-DD`) of right now.
pub fn local_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_is_after_2023() {
        assert!(now_ms() > 1_700_000_000_000);
    }

    #[test]
    fn local_day_formats_a_date() {
        let day = local_day(now_ms()).expect("current time has a local day");
        assert_eq!(day.len(), 10);
        assert_eq!(&day[4..5], "-");
        assert_eq!(local_today().len(), 10);
    }
}
