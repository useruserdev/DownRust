// SPDX-License-Identifier: GPL-3.0-or-later
//! Speed limits that change with the time of day and the day of the week.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeedSchedule {}
