// SPDX-License-Identifier: GPL-3.0-or-later
//! Result of the virus scan that runs after a download completes.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanState {
    Scanning,
    Clean,
    Threat,
    Failed,
}

impl ScanState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scanning => "scanning",
            Self::Clean => "clean",
            Self::Threat => "threat",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "scanning" => Some(Self::Scanning),
            "clean" => Some(Self::Clean),
            "threat" => Some(Self::Threat),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}
