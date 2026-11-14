// SPDX-License-Identifier: GPL-3.0-or-later
//! Statistics of completed downloads, as sent to the interface.
use serde::{Deserialize, Serialize};

use super::Category;

/// Totals of one local calendar day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsDay {
    /// Local calendar date, `YYYY-MM-DD`.
    pub day: String,
    pub bytes: u64,
    pub files: u64,
}

/// Totals of one category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsCategory {
    pub category: Category,
    pub bytes: u64,
    pub files: u64,
}

/// Byte and file counts without a date or category.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsTotals {
    pub bytes: u64,
    pub files: u64,
}

/// Complete statistics: the last 30 local days (oldest first, zero-filled), every category, and
/// the all-time totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSnapshot {
    pub days: Vec<StatsDay>,
    pub by_category: Vec<StatsCategory>,
    pub all_time: StatsTotals,
}
