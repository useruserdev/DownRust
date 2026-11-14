// SPDX-License-Identifier: GPL-3.0-or-later
//! Persisted download record and the input used to create one.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Category, ChecksumState, DownloadKind, DownloadStatus, ScanState};

pub type DownloadId = i64;
pub const PART_EXTENSION: &str = ".downrust";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeaderPair {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: DownloadId,
    pub url: String,
    pub final_url: Option<String>,
    pub referrer: Option<String>,
    pub file_name: String,
    pub save_dir: String,
    pub category: Category,
    pub status: DownloadStatus,
    pub total_size: Option<u64>,
    pub downloaded: u64,
    pub resumable: bool,
    pub connections: u8,
    pub speed_limit: Option<u64>, // bytes per second, None = unlimited
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub error: Option<String>,
    pub queue_position: i64,
    pub headers: Vec<HeaderPair>,
    pub created_at: i64, // unix ms
    pub completed_at: Option<i64>,
    pub expected_sha256: Option<String>, // lowercase hex, 64 characters
    pub checksum: Option<ChecksumState>,
    pub scan: Option<ScanState>,
    pub kind: DownloadKind,
}

impl DownloadRecord {
    pub fn final_path(&self) -> PathBuf {
        Path::new(&self.save_dir).join(&self.file_name)
    }

    pub fn part_path(&self) -> PathBuf {
        Path::new(&self.save_dir).join(format!("{}{}", self.file_name, PART_EXTENSION))
    }

    /// `headers` plus `Referer: <referrer>` when a referrer is set and no Referer header exists
    /// (case-insensitive name match).
    pub fn request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = self.headers.clone();
        if let Some(referrer) = &self.referrer {
            let has_referer = headers
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case("referer"));
            if !has_referer {
                headers.push(HeaderPair {
                    name: "Referer".to_owned(),
                    value: referrer.clone(),
                });
            }
        }
        headers
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewDownload {
    pub url: String,
    pub final_url: Option<String>,
    pub referrer: Option<String>,
    pub file_name: String,
    pub save_dir: String,
    pub category: Category,
    pub status: DownloadStatus,
    pub total_size: Option<u64>,
    pub resumable: bool,
    pub connections: u8,
    pub speed_limit: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub headers: Vec<HeaderPair>,
    pub queue_position: i64,
    pub created_at: i64,
    pub expected_sha256: Option<String>,
    pub kind: DownloadKind,
}
