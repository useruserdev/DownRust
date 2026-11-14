// SPDX-License-Identifier: GPL-3.0-or-later
//! Statistics snapshot: the last 30 local days, every category and the all-time totals.
use chrono::{Days, Local};

use super::DownloadManager;
use crate::error::Result;
use crate::model::{Category, StatsCategory, StatsDay, StatsSnapshot, StatsTotals};

/// Number of local days in the daily series, today included.
const DAY_COUNT: u64 = 30;

impl DownloadManager {
    /// Statistics of completed downloads. Days without completions are zero-filled.
    pub fn stats(&self) -> Result<StatsSnapshot> {
        let today = Local::now().date_naive();
        let first = today
            .checked_sub_days(Days::new(DAY_COUNT - 1))
            .unwrap_or(today);
        let stored = self
            .shared
            .store
            .daily_stats_since(&first.format("%Y-%m-%d").to_string())?;
        let days = (0..DAY_COUNT)
            .map(|offset| {
                let day = first
                    .checked_add_days(Days::new(offset))
                    .unwrap_or(first)
                    .format("%Y-%m-%d")
                    .to_string();
                match stored.iter().find(|entry| entry.day == day) {
                    Some(entry) => entry.clone(),
                    None => StatsDay {
                        day,
                        bytes: 0,
                        files: 0,
                    },
                }
            })
            .collect();
        let totals = self.shared.store.category_stats()?;
        let by_category: Vec<StatsCategory> = Category::ALL
            .iter()
            .map(|&category| {
                let found = totals
                    .iter()
                    .find(|(name, _)| *name == category)
                    .map(|(_, totals)| *totals)
                    .unwrap_or_default();
                StatsCategory {
                    category,
                    bytes: found.bytes,
                    files: found.files,
                }
            })
            .collect();
        let all_time = by_category
            .iter()
            .fold(StatsTotals::default(), |sum, entry| StatsTotals {
                bytes: sum.bytes.saturating_add(entry.bytes),
                files: sum.files.saturating_add(entry.files),
            });
        Ok(StatsSnapshot {
            days,
            by_category,
            all_time,
        })
    }
}
