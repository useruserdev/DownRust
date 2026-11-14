// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings for video sites downloaded with yt-dlp.
use serde::{Deserialize, Serialize};

/// Quality of a video site download. Audio only saves the sound track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteFormat {
    #[default]
    #[serde(rename = "best")]
    Best,
    #[serde(rename = "2160")]
    P2160,
    #[serde(rename = "1440")]
    P1440,
    #[serde(rename = "1080")]
    P1080,
    #[serde(rename = "720")]
    P720,
    #[serde(rename = "480")]
    P480,
    #[serde(rename = "audio")]
    AudioOnly,
}

impl SiteFormat {
    /// The largest video height the format allows. `None` for best quality and audio only.
    pub fn max_height(self) -> Option<u32> {
        match self {
            Self::Best | Self::AudioOnly => None,
            Self::P2160 => Some(2160),
            Self::P1440 => Some(1440),
            Self::P1080 => Some(1080),
            Self::P720 => Some(720),
            Self::P480 => Some(480),
        }
    }

    /// True when only the sound track is wanted.
    pub fn is_audio(self) -> bool {
        self == Self::AudioOnly
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SiteSettings {
    /// Full path to yt-dlp. Empty means: look in the tools folder, next to the program, then in PATH.
    pub ytdlp_path: String,
    /// Folder for the downloaded yt-dlp program. The app fills it at each start.
    pub tools_dir: String,
    /// Download yt-dlp when no copy is found.
    pub auto_download: bool,
    /// Run `yt-dlp -U` once a week.
    pub auto_update: bool,
    pub default_format: SiteFormat,
}

impl Default for SiteSettings {
    fn default() -> Self {
        Self {
            ytdlp_path: String::new(),
            tools_dir: String::new(),
            auto_download: true,
            auto_update: true,
            default_format: SiteFormat::Best,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_names_round_trip() {
        let text = serde_json::to_string(&SiteFormat::P1080).expect("serializes");
        assert_eq!(text, "\"1080\"");
        let audio: SiteFormat = serde_json::from_str("\"audio\"").expect("parses");
        assert_eq!(audio, SiteFormat::AudioOnly);
        assert!(serde_json::from_str::<SiteFormat>("\"4k\"").is_err());
    }

    #[test]
    fn defaults_turn_on_download_and_update() {
        let settings = SiteSettings::default();
        assert!(settings.auto_download && settings.auto_update);
        assert_eq!(settings.default_format.max_height(), None);
        assert_eq!(SiteFormat::P720.max_height(), Some(720));
    }
}
