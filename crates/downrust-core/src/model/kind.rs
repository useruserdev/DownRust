// SPDX-License-Identifier: GPL-3.0-or-later
//! Kind of transfer: a plain file, an HLS playlist or a DASH manifest.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadKind {
    #[default]
    File,
    Hls,
    Dash,
}

impl DownloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Hls => "hls",
            Self::Dash => "dash",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "file" => Some(Self::File),
            "hls" => Some(Self::Hls),
            "dash" => Some(Self::Dash),
            _ => None,
        }
    }

    /// True for HLS and DASH transfers, which download many small parts.
    pub fn is_media(self) -> bool {
        !matches!(self, Self::File)
    }

    /// Guesses the kind from the path of a URL: `.m3u8` is HLS and `.mpd` is DASH.
    /// The query string and the fragment are ignored.
    pub fn from_url(url: &str) -> Option<Self> {
        let path = url.split(['?', '#']).next().unwrap_or_default();
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".m3u8") {
            Some(Self::Hls)
        } else if lower.ends_with(".mpd") {
            Some(Self::Dash)
        } else {
            None
        }
    }

    /// Guesses the kind from a Content-Type header value. Parameters after `;` are ignored.
    pub fn from_content_type(value: &str) -> Option<Self> {
        let essence = value.split(';').next().unwrap_or_default().trim();
        match essence.to_ascii_lowercase().as_str() {
            "application/vnd.apple.mpegurl" | "application/x-mpegurl" | "audio/mpegurl" => {
                Some(Self::Hls)
            }
            "application/dash+xml" => Some(Self::Dash),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kind_from_url_path_only() {
        assert_eq!(
            DownloadKind::from_url("https://a.test/x/index.m3u8?token=1"),
            Some(DownloadKind::Hls)
        );
        assert_eq!(
            DownloadKind::from_url("https://a.test/Live/MANIFEST.MPD#t"),
            Some(DownloadKind::Dash)
        );
        assert_eq!(
            DownloadKind::from_url("https://a.test/file.zip?x=.m3u8"),
            None
        );
    }

    #[test]
    fn detects_kind_from_content_type() {
        assert_eq!(
            DownloadKind::from_content_type("application/vnd.apple.mpegurl; charset=utf-8"),
            Some(DownloadKind::Hls)
        );
        assert_eq!(
            DownloadKind::from_content_type("Application/X-MPEGURL"),
            Some(DownloadKind::Hls)
        );
        assert_eq!(
            DownloadKind::from_content_type("application/dash+xml"),
            Some(DownloadKind::Dash)
        );
        assert_eq!(DownloadKind::from_content_type("video/mp4"), None);
    }

    #[test]
    fn text_round_trips() {
        for kind in [DownloadKind::File, DownloadKind::Hls, DownloadKind::Dash] {
            assert_eq!(DownloadKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(DownloadKind::parse("ftp"), None);
    }
}
