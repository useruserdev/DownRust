// SPDX-License-Identifier: GPL-3.0-or-later
//! URL detection helpers.
use percent_encoding::percent_decode_str;
use url::Url;

use super::filename::split_extension;
use crate::model::Category;

const MAX_URL_CHARS: usize = 2048;

/// True when the text is an http or https address without spaces whose last path segment
/// has an extension that maps to a category other than `Other`.
pub fn looks_like_download_url(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_URL_CHARS
        || trimmed.chars().any(char::is_whitespace)
    {
        return false;
    }
    let Ok(url) = Url::parse(trimmed) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let Some(segment) = url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|s| !s.is_empty()))
    else {
        return false;
    };
    let decoded = percent_decode_str(segment).decode_utf8_lossy();
    match split_extension(&decoded).1 {
        Some(ext) => Category::from_extension(ext) != Category::Other,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_downloads() {
        assert!(looks_like_download_url(
            "https://example.com/files/setup.exe"
        ));
        assert!(looks_like_download_url("http://example.com/movie.mkv"));
        assert!(looks_like_download_url(
            "https://example.com/a.zip?token=1#x"
        ));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert!(looks_like_download_url("  https://example.com/movie.mkv  "));
    }

    #[test]
    fn rejects_paths_without_a_category_extension() {
        assert!(!looks_like_download_url("https://example.com/"));
        assert!(!looks_like_download_url("https://example.com/page.html"));
        assert!(!looks_like_download_url("https://example.com/file."));
        assert!(!looks_like_download_url("https://example.com/.mp4"));
        assert!(!looks_like_download_url("https://example.com/dir/noext"));
    }

    #[test]
    fn rejects_other_schemes_and_non_urls() {
        assert!(!looks_like_download_url("ftp://example.com/a.zip"));
        assert!(!looks_like_download_url("file:///C:/a.zip"));
        assert!(!looks_like_download_url("not a url"));
        assert!(!looks_like_download_url("example.com/a.zip"));
        assert!(!looks_like_download_url(""));
    }

    #[test]
    fn rejects_whitespace_inside_and_long_text() {
        assert!(!looks_like_download_url("https://example.com/a zip.zip"));
        assert!(!looks_like_download_url(
            "https://example.com/a.zip with text"
        ));
        let long = format!("https://example.com/{}.zip", "a".repeat(2100));
        assert!(!looks_like_download_url(&long));
    }

    #[test]
    fn decodes_the_segment_before_checking_the_extension() {
        assert!(looks_like_download_url("https://example.com/report%2Epdf"));
    }
}
