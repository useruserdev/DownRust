// SPDX-License-Identifier: GPL-3.0-or-later
//! File categories and the extensions that select them.
use serde::{Deserialize, Serialize};

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "3gp", "ts", "m2ts",
    "vob", "ogv",
];
const MUSIC_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "aac", "flac", "wav", "ogg", "oga", "opus", "wma", "aiff", "aif", "alac", "mka",
];
const ARCHIVE_EXTENSIONS: &[&str] = &[
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "lz", "lzma", "cab",
    "iso", "img",
];
const PROGRAM_EXTENSIONS: &[&str] = &[
    "exe",
    "msi",
    "msix",
    "msixbundle",
    "appx",
    "appxbundle",
    "dmg",
    "pkg",
    "deb",
    "rpm",
    "apk",
    "appimage",
    "jar",
    "bat",
    "cmd",
    "ps1",
    "sh",
];
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "csv",
    "epub", "mobi", "md", "json", "xml",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Video,
    Music,
    Archives,
    Programs,
    Documents,
    Other,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Category::Video,
        Category::Music,
        Category::Archives,
        Category::Programs,
        Category::Documents,
        Category::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Music => "music",
            Self::Archives => "archives",
            Self::Programs => "programs",
            Self::Documents => "documents",
            Self::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "video" => Some(Self::Video),
            "music" => Some(Self::Music),
            "archives" => Some(Self::Archives),
            "programs" => Some(Self::Programs),
            "documents" => Some(Self::Documents),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    /// Folder name under the download root. `Other` has none and uses the root itself.
    pub fn folder_name(self) -> Option<&'static str> {
        match self {
            Self::Video => Some("Video"),
            Self::Music => Some("Music"),
            Self::Archives => Some("Archives"),
            Self::Programs => Some("Programs"),
            Self::Documents => Some("Documents"),
            Self::Other => None,
        }
    }

    /// Case-insensitive lookup by extension without the leading dot.
    pub fn from_extension(ext: &str) -> Category {
        let ext = ext.to_ascii_lowercase();
        let ext = ext.as_str();
        if VIDEO_EXTENSIONS.contains(&ext) {
            Category::Video
        } else if MUSIC_EXTENSIONS.contains(&ext) {
            Category::Music
        } else if ARCHIVE_EXTENSIONS.contains(&ext) {
            Category::Archives
        } else if PROGRAM_EXTENSIONS.contains(&ext) {
            Category::Programs
        } else if DOCUMENT_EXTENSIONS.contains(&ext) {
            Category::Documents
        } else {
            Category::Other
        }
    }

    /// Uses the text after the last dot. A name without a dot is `Other`.
    pub fn from_file_name(name: &str) -> Category {
        match name.rsplit_once('.') {
            Some((_, ext)) => Self::from_extension(ext),
            None => Category::Other,
        }
    }
}
