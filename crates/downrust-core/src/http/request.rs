// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP request building.
use reqwest::header::{ACCEPT_ENCODING, IF_RANGE, RANGE};

use crate::model::HeaderPair;

/// Builds a GET request. It always asks for an unencoded body and adds the extra headers.
/// A range is `(start, end)` where `end` is inclusive, or `None` for an open end.
/// `If-Range` is sent only together with a range.
pub fn get_request(
    client: &reqwest::Client,
    url: &str,
    headers: &[HeaderPair],
    range: Option<(u64, Option<u64>)>,
    if_range: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
    for header in headers {
        request = request.header(header.name.as_str(), header.value.as_str());
    }
    if let Some((start, end)) = range {
        let value = match end {
            Some(end) => format!("bytes={start}-{end}"),
            None => format!("bytes={start}-"),
        };
        request = request.header(RANGE, value);
        if let Some(tag) = if_range {
            request = request.header(IF_RANGE, tag);
        }
    }
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(
        range: Option<(u64, Option<u64>)>,
        if_range: Option<&str>,
        headers: &[HeaderPair],
    ) -> reqwest::Request {
        let client = reqwest::Client::new();
        get_request(
            &client,
            "https://example.com/file.bin",
            headers,
            range,
            if_range,
        )
        .build()
        .expect("request builds")
    }

    fn header<'a>(request: &'a reqwest::Request, name: &str) -> Option<&'a str> {
        request.headers().get(name).and_then(|v| v.to_str().ok())
    }

    #[test]
    fn closed_range_and_if_range() {
        let request = build(Some((10, Some(19))), Some("\"v1\""), &[]);
        assert_eq!(header(&request, "range"), Some("bytes=10-19"));
        assert_eq!(header(&request, "if-range"), Some("\"v1\""));
    }

    #[test]
    fn open_range_without_if_range() {
        let request = build(Some((5, None)), None, &[]);
        assert_eq!(header(&request, "range"), Some("bytes=5-"));
        assert_eq!(header(&request, "if-range"), None);
    }

    #[test]
    fn no_range_sends_no_range_headers() {
        let request = build(None, Some("\"v1\""), &[]);
        assert_eq!(header(&request, "range"), None);
        assert_eq!(header(&request, "if-range"), None);
    }

    #[test]
    fn always_asks_for_identity_encoding() {
        let request = build(None, None, &[]);
        assert_eq!(header(&request, "accept-encoding"), Some("identity"));
    }

    #[test]
    fn adds_extra_headers() {
        let extra = [HeaderPair {
            name: "Referer".to_owned(),
            value: "https://example.com/".to_owned(),
        }];
        let request = build(None, None, &extra);
        assert_eq!(header(&request, "referer"), Some("https://example.com/"));
    }
}
