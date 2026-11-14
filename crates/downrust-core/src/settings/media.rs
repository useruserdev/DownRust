// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings for HLS and DASH transfers and the ffmpeg helper.
use serde::{Deserialize, Serialize};

/// Which variant of an adaptive stream is downloaded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaQuality {
    /// Highest BANDWIDTH variant.
    #[default]
    Best,
    /// Lowest BANDWIDTH variant.
    Worst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MediaSettings {
    /// Full path to ffmpeg. Empty means: look next to the program, then in PATH.
    pub ffmpeg_path: String,
    /// Convert a finished MPEG-TS file to MP4 with ffmpeg (stream copy, no re-encoding).
    pub convert_to_mp4: bool,
    pub quality: MediaQuality,
}

impl Default for MediaSettings {
    fn default() -> Self {
        Self {
            ffmpeg_path: String::new(),
            convert_to_mp4: true,
            quality: MediaQuality::Best,
        }
    }
}

impl MediaSettings {
    /// Removes spaces around the ffmpeg path.
    pub fn sanitized(mut self) -> Self {
        self.ffmpeg_path = self.ffmpeg_path.trim().to_owned();
        self
    }
}
