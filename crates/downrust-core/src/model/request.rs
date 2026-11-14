// SPDX-License-Identifier: GPL-3.0-or-later
//! Input for adding a download and the suggested save target.
use serde::{Deserialize, Serialize};

use super::{Category, DownloadKind, HeaderPair, ProbeInfo};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AddRequest {
    pub url: String,
    pub file_name: Option<String>,
    pub save_dir: Option<String>,
    pub connections: Option<u8>,
    pub speed_limit: Option<u64>,
    pub start: bool, // true means Queued, false means Paused
    pub referrer: Option<String>,
    pub headers: Vec<HeaderPair>,
    pub probe: Option<ProbeInfo>, // result the Add dialog already fetched
    pub expected_sha256: Option<String>, // checked after the transfer when set
    pub kind: Option<DownloadKind>, // None means detect from the address and the response type
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSuggestion {
    pub category: Category,
    pub save_dir: String,
    pub file_name: String,
}
