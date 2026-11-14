// SPDX-License-Identifier: GPL-3.0-or-later
//! Direction of a queue move.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueMove {
    Up,
    Down,
    Top,
    Bottom,
}
