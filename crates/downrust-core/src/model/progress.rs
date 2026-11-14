// SPDX-License-Identifier: GPL-3.0-or-later
//! Live progress values sent to the interface.
use serde::{Deserialize, Serialize};

use super::{DownloadId, Phase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentView {
    pub start: u64,
    pub end: u64,
    pub done: u64,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressItem {
    pub id: DownloadId,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: u64,
    pub eta_secs: Option<u64>,
    pub connections: u32,
    pub phase: Phase,
    pub segments: Vec<SegmentView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressBatch {
    pub items: Vec<ProgressItem>,
    pub total_speed: u64,
}
