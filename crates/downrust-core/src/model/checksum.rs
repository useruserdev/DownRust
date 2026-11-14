// SPDX-License-Identifier: GPL-3.0-or-later
//! Checksum state of a download and the result of a checksum comparison.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumState {
    Verifying,
    Ok,
    Mismatch,
}

impl ChecksumState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verifying => "verifying",
            Self::Ok => "ok",
            Self::Mismatch => "mismatch",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "verifying" => Some(Self::Verifying),
            "ok" => Some(Self::Ok),
            "mismatch" => Some(Self::Mismatch),
            _ => None,
        }
    }
}

/// Outcome of comparing a finished file with an expected SHA-256 value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecksumReport {
    pub actual: String,
    pub matches: bool,
}
