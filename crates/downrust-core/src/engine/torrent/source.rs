// SPDX-License-Identifier: GPL-3.0-or-later
//! Finds out where a torrent comes from and what it is called.
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use url::Url;

use crate::error::{Error, Result};

const FALLBACK_NAME: &str = "torrent";

/// Where the torrent description comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Magnet(String),
    Http(String),
    File(PathBuf),
}

/// Picks the source from the address the user gave.
pub(crate) fn detect(address: &str) -> Result<Source> {
    let address = address.trim();
    if address
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("magnet:"))
    {
        return Ok(Source::Magnet(address.to_owned()));
    }
    if address.starts_with("http://") || address.starts_with("https://") {
        return Ok(Source::Http(address.to_owned()));
    }
    if address
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file://"))
    {
        let url = Url::parse(address).map_err(|_| Error::InvalidUrl(address.to_owned()))?;
        let path = url
            .to_file_path()
            .map_err(|()| Error::InvalidUrl(address.to_owned()))?;
        return Ok(Source::File(path));
    }
    if address.is_empty() {
        return Err(Error::InvalidUrl(address.to_owned()));
    }
    Ok(Source::File(PathBuf::from(address)))
}

/// File name used when the request gives none.
pub(crate) fn initial_name(address: &str) -> String {
    match detect(address) {
        Ok(Source::Magnet(link)) => magnet_name(&link),
        Ok(Source::Http(url)) => url_stem(&url),
        Ok(Source::File(path)) => stem_or_fallback(&path),
        Err(_) => FALLBACK_NAME.to_owned(),
    }
}

/// The `dn` value of a magnet link, percent-decoded.
pub(crate) fn magnet_name(link: &str) -> String {
    let Ok(url) = Url::parse(link) else {
        return FALLBACK_NAME.to_owned();
    };
    url.query_pairs()
        .find(|(key, value)| key == "dn" && !value.trim().is_empty())
        .map(|(_, value)| value.trim().to_owned())
        .unwrap_or_else(|| FALLBACK_NAME.to_owned())
}

/// The `.torrent` file stem taken from the last segment of an address.
fn url_stem(address: &str) -> String {
    let Ok(url) = Url::parse(address) else {
        return FALLBACK_NAME.to_owned();
    };
    let segment = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or_default();
    let decoded = percent_decode_str(segment).decode_utf8_lossy();
    stem_or_fallback(Path::new(decoded.as_ref()))
}

fn stem_or_fallback(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| FALLBACK_NAME.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnet_name_is_percent_decoded() {
        let link =
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=My%20Film%2B2026";
        assert_eq!(magnet_name(link), "My Film+2026");
    }

    #[test]
    fn magnet_without_name_uses_fallback() {
        assert_eq!(magnet_name("magnet:?xt=urn:btih:abc"), "torrent");
        assert_eq!(magnet_name("magnet:?dn=%20"), "torrent");
    }

    #[test]
    fn sources_are_detected() {
        assert_eq!(
            detect("MAGNET:?xt=urn:btih:abc").expect("magnet"),
            Source::Magnet("MAGNET:?xt=urn:btih:abc".to_owned())
        );
        assert_eq!(
            detect("https://example.com/a.torrent").expect("http"),
            Source::Http("https://example.com/a.torrent".to_owned())
        );
        assert_eq!(
            detect("C:\\files\\a.torrent").expect("path"),
            Source::File(PathBuf::from("C:\\files\\a.torrent"))
        );
        assert!(matches!(
            detect("file:///C:/files/a.torrent").expect("file url"),
            Source::File(_)
        ));
        assert!(detect("   ").is_err());
    }

    #[test]
    fn names_come_from_the_address() {
        assert_eq!(initial_name("magnet:?xt=urn:btih:abc&dn=Film"), "Film");
        assert_eq!(
            initial_name("https://example.com/dl/My%20Set.torrent"),
            "My Set"
        );
        assert_eq!(initial_name("C:\\files\\ubuntu.torrent"), "ubuntu");
        assert_eq!(initial_name("https://example.com/"), "torrent");
    }
}
