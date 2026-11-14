// SPDX-License-Identifier: GPL-3.0-or-later
//! Kind of transfer: a plain file, an HLS playlist, a DASH manifest, a video page read by
//! yt-dlp, a torrent, or an FTP or SFTP file.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadKind {
    #[default]
    File,
    Hls,
    Dash,
    /// A page of a video site; yt-dlp finds and downloads the video.
    Site,
    /// A magnet link or a `.torrent` file.
    Torrent,
    /// An `ftp://`, `ftps://` or `sftp://` address.
    Ftp,
}

impl DownloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Hls => "hls",
            Self::Dash => "dash",
            Self::Site => "site",
            Self::Torrent => "torrent",
            Self::Ftp => "ftp",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "file" => Some(Self::File),
            "hls" => Some(Self::Hls),
            "dash" => Some(Self::Dash),
            "site" => Some(Self::Site),
            "torrent" => Some(Self::Torrent),
            "ftp" => Some(Self::Ftp),
            _ => None,
        }
    }

    /// True for HLS and DASH transfers, which download many small parts.
    pub fn is_media(self) -> bool {
        matches!(self, Self::Hls | Self::Dash)
    }

    /// True for kinds that are not fetched over HTTP by the DownRust engine itself, so the
    /// address is never probed with an HTTP request.
    pub fn is_external(self) -> bool {
        matches!(self, Self::Site | Self::Torrent | Self::Ftp)
    }

    /// Guesses the kind from a URL: a `magnet:` link or a `.torrent` path is a torrent,
    /// `ftp://`, `ftps://` and `sftp://` are FTP, `.m3u8` is HLS and `.mpd` is DASH.
    /// The query string and the fragment are ignored.
    pub fn from_url(url: &str) -> Option<Self> {
        let trimmed = url.trim();
        let scheme = trimmed
            .split_once(':')
            .map(|(scheme, _)| scheme.to_ascii_lowercase())
            .unwrap_or_default();
        match scheme.as_str() {
            "magnet" => return Some(Self::Torrent),
            "ftp" | "ftps" | "sftp" => return Some(Self::Ftp),
            _ => {}
        }
        let path = trimmed.split(['?', '#']).next().unwrap_or_default();
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".torrent") {
            Some(Self::Torrent)
        } else if lower.ends_with(".m3u8") {
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
    fn detects_torrent_and_ftp_addresses() {
        assert_eq!(
            DownloadKind::from_url("magnet:?xt=urn:btih:abc"),
            Some(DownloadKind::Torrent)
        );
        assert_eq!(
            DownloadKind::from_url("https://a.test/ubuntu.iso.torrent?x=1"),
            Some(DownloadKind::Torrent)
        );
        assert_eq!(
            DownloadKind::from_url("FTP://host/pub/file.zip"),
            Some(DownloadKind::Ftp)
        );
        assert_eq!(
            DownloadKind::from_url("sftp://user@host/file"),
            Some(DownloadKind::Ftp)
        );
        assert!(DownloadKind::Torrent.is_external());
        assert!(!DownloadKind::Hls.is_external());
        assert!(!DownloadKind::Site.is_media());
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
        for kind in [
            DownloadKind::File,
            DownloadKind::Hls,
            DownloadKind::Dash,
            DownloadKind::Site,
            DownloadKind::Torrent,
            DownloadKind::Ftp,
        ] {
            assert_eq!(DownloadKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(DownloadKind::parse("gopher"), None);
    }
}
