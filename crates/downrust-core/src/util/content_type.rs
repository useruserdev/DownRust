// SPDX-License-Identifier: GPL-3.0-or-later
//! Content-Type header helpers.

/// Maps a `Content-Type` value to a file extension. Parameters are ignored and case does not matter.
/// `application/octet-stream` and unknown types give `None`.
pub fn extension_for_content_type(value: &str) -> Option<&'static str> {
    let base = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match base.as_str() {
        "application/pdf" => Some("pdf"),
        "application/zip" | "application/x-zip-compressed" => Some("zip"),
        "application/x-7z-compressed" => Some("7z"),
        "application/vnd.rar" | "application/x-rar-compressed" => Some("rar"),
        "application/gzip" | "application/x-gzip" => Some("gz"),
        "application/x-tar" => Some("tar"),
        "application/x-msdownload"
        | "application/vnd.microsoft.portable-executable"
        | "application/x-msdos-program" => Some("exe"),
        "application/x-msi" | "application/x-ms-installer" => Some("msi"),
        "application/x-iso9660-image" => Some("iso"),
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/x-matroska" => Some("mkv"),
        "video/quicktime" => Some("mov"),
        "audio/mpeg" => Some("mp3"),
        "audio/mp4" => Some("m4a"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/ogg" => Some("ogg"),
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "text/plain" => Some("txt"),
        "text/html" => Some("html"),
        "text/csv" => Some("csv"),
        "application/json" => Some("json"),
        "application/xml" | "text/xml" => Some("xml"),
        "application/epub+zip" => Some("epub"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_types() {
        let table = [
            ("application/pdf", "pdf"),
            ("application/zip", "zip"),
            ("application/x-zip-compressed", "zip"),
            ("application/x-7z-compressed", "7z"),
            ("application/vnd.rar", "rar"),
            ("application/x-rar-compressed", "rar"),
            ("application/gzip", "gz"),
            ("application/x-gzip", "gz"),
            ("application/x-tar", "tar"),
            ("application/x-msdownload", "exe"),
            ("application/vnd.microsoft.portable-executable", "exe"),
            ("application/x-msdos-program", "exe"),
            ("application/x-msi", "msi"),
            ("application/x-ms-installer", "msi"),
            ("application/x-iso9660-image", "iso"),
            ("video/mp4", "mp4"),
            ("video/webm", "webm"),
            ("video/x-matroska", "mkv"),
            ("video/quicktime", "mov"),
            ("audio/mpeg", "mp3"),
            ("audio/mp4", "m4a"),
            ("audio/flac", "flac"),
            ("audio/x-flac", "flac"),
            ("audio/wav", "wav"),
            ("audio/x-wav", "wav"),
            ("audio/ogg", "ogg"),
            ("image/png", "png"),
            ("image/jpeg", "jpg"),
            ("image/gif", "gif"),
            ("image/webp", "webp"),
            ("text/plain", "txt"),
            ("text/html", "html"),
            ("text/csv", "csv"),
            ("application/json", "json"),
            ("application/xml", "xml"),
            ("text/xml", "xml"),
            ("application/epub+zip", "epub"),
        ];
        for (mime, ext) in table {
            assert_eq!(extension_for_content_type(mime), Some(ext), "{mime}");
        }
    }

    #[test]
    fn ignores_parameters_and_case() {
        assert_eq!(
            extension_for_content_type("Application/PDF; charset=binary"),
            Some("pdf")
        );
        assert_eq!(extension_for_content_type("  video/MP4 ;x=1"), Some("mp4"));
    }

    #[test]
    fn never_maps_octet_stream_or_unknown_types() {
        assert_eq!(extension_for_content_type("application/octet-stream"), None);
        assert_eq!(extension_for_content_type("application/x-unknown"), None);
        assert_eq!(extension_for_content_type(""), None);
    }
}
