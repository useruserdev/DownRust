// SPDX-License-Identifier: GPL-3.0-or-later
//! Address probing over HTTP.
use std::time::Duration;

use reqwest::header::{CONTENT_DISPOSITION, CONTENT_RANGE, CONTENT_TYPE, ETAG, LAST_MODIFIED};
use reqwest::Response;
use url::Url;

use super::request::get_request;
use crate::error::{Error, Result};
use crate::model::{Category, HeaderPair, ProbeInfo};
use crate::util::content_disposition::filename_from_content_disposition;
use crate::util::content_range::{parse_content_range, parse_unsatisfied_total};
use crate::util::content_type::extension_for_content_type;
use crate::util::filename::{file_name_from_url, sanitize_file_name, split_extension};

const FALLBACK_NAME: &str = "download";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sizing {
    total: Option<u64>,
    resumable: bool,
}

/// Probes an address with a one-byte range request and reports its size, resume support and name.
pub async fn probe(
    client: &reqwest::Client,
    url: &str,
    headers: &[HeaderPair],
    timeout: Duration,
) -> Result<ProbeInfo> {
    let address = url.trim();
    let original = parse_address(address)?;
    let first = send_probe(client, address, headers, Some((0, Some(0))), timeout).await?;
    let (response, sizing) = match first.status().as_u16() {
        206 => {
            let sizing = sizing_partial(header_text(&first, CONTENT_RANGE).as_deref());
            (first, sizing)
        }
        200 => {
            let sizing = sizing_full(first.content_length());
            (first, sizing)
        }
        416 => {
            let sizing = sizing_unsatisfied(header_text(&first, CONTENT_RANGE).as_deref())?;
            (first, sizing)
        }
        code @ 400..=499 => {
            let retry = send_probe(client, address, headers, None, timeout).await?;
            if retry.status().as_u16() != 200 {
                return Err(Error::HttpStatus(code));
            }
            let sizing = sizing_full(retry.content_length());
            (retry, sizing)
        }
        code => return Err(Error::HttpStatus(code)),
    };
    let final_url = response.url().clone();
    let disposition = header_text(&response, CONTENT_DISPOSITION);
    let content_type = header_text(&response, CONTENT_TYPE);
    let file_name = with_extension(
        name_from_parts(disposition.as_deref(), &final_url, &original),
        content_type.as_deref(),
    );
    Ok(ProbeInfo {
        url: address.to_owned(),
        final_url: final_url.to_string(),
        category: Category::from_file_name(&file_name),
        file_name,
        total_size: sizing.total,
        resumable: sizing.resumable,
        content_type,
        etag: header_text(&response, ETAG),
        last_modified: header_text(&response, LAST_MODIFIED),
    })
}

async fn send_probe(
    client: &reqwest::Client,
    url: &str,
    headers: &[HeaderPair],
    range: Option<(u64, Option<u64>)>,
    timeout: Duration,
) -> Result<Response> {
    let pending = get_request(client, url, headers, range, None).send();
    match tokio::time::timeout(timeout, pending).await {
        Ok(response) => Ok(response?),
        Err(_) => Err(Error::Timeout(format!("no response within {timeout:?}"))),
    }
}

fn parse_address(text: &str) -> Result<Url> {
    let parsed = Url::parse(text).map_err(|_| Error::InvalidUrl(text.to_owned()))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        other => Err(Error::UnsupportedScheme(other.to_owned())),
    }
}

fn header_text(response: &Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(&name)
        .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
}

fn sizing_partial(content_range: Option<&str>) -> Sizing {
    let total = content_range
        .and_then(parse_content_range)
        .and_then(|range| range.total);
    Sizing {
        total,
        resumable: total.is_some(),
    }
}

fn sizing_full(content_length: Option<u64>) -> Sizing {
    Sizing {
        total: content_length,
        resumable: false,
    }
}

fn sizing_unsatisfied(content_range: Option<&str>) -> Result<Sizing> {
    let total = content_range
        .and_then(parse_unsatisfied_total)
        .ok_or_else(|| Error::Protocol("416 reply without a usable Content-Range".to_owned()))?;
    Ok(Sizing {
        total: Some(total),
        resumable: false,
    })
}

fn name_from_parts(disposition: Option<&str>, final_url: &Url, original: &Url) -> String {
    disposition
        .and_then(filename_from_content_disposition)
        .map(|name| sanitize_file_name(&name))
        .or_else(|| file_name_from_url(final_url))
        .or_else(|| file_name_from_url(original))
        .unwrap_or_else(|| FALLBACK_NAME.to_owned())
}

fn with_extension(name: String, content_type: Option<&str>) -> String {
    if split_extension(&name).1.is_some() {
        return name;
    }
    match content_type.and_then(extension_for_content_type) {
        Some(ext) => sanitize_file_name(&format!("{name}.{ext}")),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).expect("test url is valid")
    }

    #[test]
    fn parse_address_accepts_http_and_https() {
        assert!(parse_address("https://example.com/a.bin").is_ok());
        assert!(parse_address("http://example.com/a.bin").is_ok());
    }

    #[test]
    fn parse_address_rejects_other_schemes_and_invalid_text() {
        assert!(matches!(
            parse_address("ftp://example.com/a"),
            Err(Error::UnsupportedScheme(ref s)) if s == "ftp"
        ));
        assert!(matches!(
            parse_address("example.com/a"),
            Err(Error::InvalidUrl(_))
        ));
    }

    #[test]
    fn partial_reply_reads_total_and_resumability() {
        assert_eq!(
            sizing_partial(Some("bytes 0-0/1000")),
            Sizing {
                total: Some(1000),
                resumable: true
            }
        );
    }

    #[test]
    fn partial_reply_with_unknown_total_is_not_resumable() {
        assert_eq!(
            sizing_partial(Some("bytes 0-0/*")),
            Sizing {
                total: None,
                resumable: false
            }
        );
        assert_eq!(
            sizing_partial(None),
            Sizing {
                total: None,
                resumable: false
            }
        );
    }

    #[test]
    fn full_reply_uses_content_length_and_is_not_resumable() {
        assert_eq!(
            sizing_full(Some(42)),
            Sizing {
                total: Some(42),
                resumable: false
            }
        );
        assert_eq!(
            sizing_full(None),
            Sizing {
                total: None,
                resumable: false
            }
        );
    }

    #[test]
    fn unsatisfied_reply_needs_a_total() {
        assert_eq!(
            sizing_unsatisfied(Some("bytes */500")).ok(),
            Some(Sizing {
                total: Some(500),
                resumable: false
            })
        );
        assert!(sizing_unsatisfied(Some("bytes 0-0/500")).is_err());
        assert!(sizing_unsatisfied(None).is_err());
    }

    #[test]
    fn name_prefers_disposition_then_final_url_then_original() {
        let final_url = url("https://example.com/files/a%20b.zip");
        let original = url("https://example.com/x/original.bin");
        assert_eq!(
            name_from_parts(
                Some("attachment; filename=\"report.pdf\""),
                &final_url,
                &original
            ),
            "report.pdf"
        );
        assert_eq!(name_from_parts(None, &final_url, &original), "a b.zip");
        assert_eq!(
            name_from_parts(None, &url("https://example.com/"), &original),
            "original.bin"
        );
        assert_eq!(
            name_from_parts(
                None,
                &url("https://example.com/"),
                &url("https://example.com/")
            ),
            "download"
        );
    }

    #[test]
    fn disposition_name_is_sanitized() {
        let final_url = url("https://example.com/");
        assert_eq!(
            name_from_parts(
                Some("attachment; filename=\"a:b?.txt\""),
                &final_url,
                &final_url
            ),
            "a_b_.txt"
        );
    }

    #[test]
    fn extension_is_added_from_content_type_when_missing() {
        assert_eq!(
            with_extension(
                "download".to_owned(),
                Some("application/pdf; charset=binary")
            ),
            "download.pdf"
        );
        assert_eq!(
            with_extension("report.pdf".to_owned(), Some("text/html")),
            "report.pdf"
        );
        assert_eq!(
            with_extension("download".to_owned(), Some("application/octet-stream")),
            "download"
        );
        assert_eq!(with_extension("download".to_owned(), None), "download");
    }
}
