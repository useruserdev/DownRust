// SPDX-License-Identifier: GPL-3.0-or-later
//! Result of probing a download address.
use serde::{Deserialize, Serialize};

use super::Category;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeInfo {
    pub url: String,
    pub final_url: String,
    pub file_name: String,
    pub total_size: Option<u64>,
    pub resumable: bool,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub category: Category,
}
